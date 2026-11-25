//! Lecture des journaux de Claude Code (portage de ClaudeIsles/server/watcher.mjs) :
//! ~/.claude/projects/<projet>/<session>.jsonl pour les sessions et les agents,
//! <session>/subagents/agent-*.meta.json pour savoir quand un agent a fini.
//! Les questions viennent du hook Notification (voir hook.rs) ou, à défaut,
//! d'un appel AskUserQuestion resté sans réponse.

use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const LOOKBACK_MS: f64 = 3.0 * 3600e3; // fichiers relus au démarrage
const SESSION_ACTIVE_MS: f64 = 15.0 * 60e3; // session affichée
const SESSION_BUSY_MS: f64 = 20e3; // Claude écrit encore : il travaille
const KEEP_DONE_MS: f64 = 120e3; // salle d'un agent terminé gardée 2 min
const STALE_MS: f64 = 90.0 * 60e3; // plus aucun signe de vie
const ASK_DELAY_MS: f64 = 2e3;
const ANSWERED_MARGIN_MS: f64 = 3e3;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum St {
    Running,
    Done,
    Failed,
    Stale,
}

#[derive(Clone, PartialEq, Debug)]
pub struct MAgent {
    pub id: String,
    pub name: String,
    pub status: St,
    pub started: f64,
}

#[derive(Clone, PartialEq, Debug)]
pub struct MSession {
    pub id: String,
    pub name: String,
    pub started: f64,
    pub busy: bool,
    pub question: Option<String>,
    /// Fen�tre qui h�berge la session (fournie par le hook), � ramener au clic.
    pub focus: Option<isize>,
    pub agents: Vec<MAgent>,
}

pub type Model = Vec<MSession>;

pub fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs_f64() * 1000.0).unwrap_or(0.0)
}

fn mtime_ms(m: &fs::Metadata) -> f64 {
    m.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64() * 1000.0)
        .unwrap_or(0.0)
}

/// "2026-11-25T10:00:00.123Z" -> millisecondes depuis 1970.
pub fn parse_ts(s: &str) -> Option<f64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let n = |a: usize, l: usize| s.get(a..a + l)?.parse::<i64>().ok();
    let (y, mo, d, h, mi, se) = (n(0, 4)?, n(5, 2)?, n(8, 2)?, n(11, 2)?, n(14, 2)?, n(17, 2)?);
    let frac = if b.get(19) == Some(&b'.') {
        let digits: String = s[20..].chars().take_while(|c| c.is_ascii_digit()).collect();
        format!("0.{digits}").parse::<f64>().unwrap_or(0.0)
    } else {
        0.0
    };
    // jours depuis 1970 (algorithme de H. Hinnant)
    let y2 = if mo <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let mp = (mo + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(((days * 86400 + h * 3600 + mi * 60 + se) as f64 + frac) * 1000.0)
}

pub fn questions_dir() -> PathBuf {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
    base.join("PokAgent").join("questions")
}

#[derive(Default)]
struct Cursor {
    offset: u64,
    rest: Vec<u8>,
    mtime: f64,
}

struct Sess {
    id: String,
    proj_dir: PathBuf,
    slug: String,
    cwd: Option<String>,
    started: f64,
    mtime: f64,
    ask: Option<(String, String, f64)>, // tool_use_id, question, horodatage
}

struct Agent {
    session: String,
    agent_id: Option<String>,
    name: String,
    status: St,
    started: f64,
    ended: f64,
    last_seen: f64,
}

pub struct Watcher {
    projects: PathBuf,
    sessions: HashMap<String, Sess>,
    agents: HashMap<String, Agent>,
    by_task: HashMap<String, String>,
    files: HashMap<PathBuf, Cursor>,
    dirs: HashMap<PathBuf, f64>,
    last_full: f64,
    started: f64,
}

fn tag<'a>(s: &'a str, name: &str) -> Option<&'a str> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let a = s.find(&open)? + open.len();
    let b = s[a..].find(&close)? + a;
    Some(s[a..b].trim())
}

fn map_status(s: Option<&str>) -> St {
    let s = s.unwrap_or("").to_lowercase();
    if s.contains("fail") || s.contains("error") || s.contains("kill") || s.contains("stop") || s.contains("cancel") {
        St::Failed
    } else {
        St::Done
    }
}

fn read_new_lines(file: &Path, cur: &mut Cursor) -> Vec<Value> {
    let Ok(meta) = fs::metadata(file) else { return vec![] };
    let size = meta.len();
    if size < cur.offset {
        cur.offset = 0;
        cur.rest.clear();
    }
    if size == cur.offset {
        return vec![];
    }
    let Ok(mut f) = fs::File::open(file) else { return vec![] };
    if f.seek(SeekFrom::Start(cur.offset)).is_err() {
        return vec![];
    }
    let mut buf = Vec::with_capacity((size - cur.offset) as usize);
    if f.take(size - cur.offset).read_to_end(&mut buf).is_err() {
        return vec![];
    }
    cur.offset += buf.len() as u64;
    let mut all = std::mem::take(&mut cur.rest);
    all.extend_from_slice(&buf);
    let Some(last) = all.iter().rposition(|&b| b == b'\n') else {
        cur.rest = all;
        return vec![];
    };
    cur.rest = all[last + 1..].to_vec();
    all[..last]
        .split(|&b| b == b'\n')
        .filter(|l| !l.iter().all(|c| c.is_ascii_whitespace()))
        .filter_map(|l| serde_json::from_slice(l).ok())
        .collect()
}

impl Watcher {
    pub fn new() -> Self {
        let home = std::env::var_os("USERPROFILE").map(PathBuf::from).unwrap_or_default();
        Watcher {
            projects: home.join(".claude").join("projects"),
            sessions: HashMap::new(),
            agents: HashMap::new(),
            by_task: HashMap::new(),
            files: HashMap::new(),
            dirs: HashMap::new(),
            last_full: 0.0,
            started: now_ms(),
        }
    }

    pub fn poll(&mut self) {
        self.scan_files();
        self.scan_agents();
        self.mark_stale();
    }

    fn scan_files(&mut self) {
        let now = now_ms();
        // Un dossier ne change de date que si un fichier y est créé : on ne relit
        // la liste que dans ce cas (et entièrement toutes les 30 s par sécurité).
        let full = now - self.last_full > 30e3;
        if full {
            self.last_full = now;
        }
        let mut new_dirs = vec![];
        if let Ok(rd) = fs::read_dir(&self.projects) {
            for e in rd.flatten() {
                let p = e.path();
                let Ok(m) = e.metadata() else { continue };
                if !m.is_dir() {
                    continue;
                }
                let mt = mtime_ms(&m);
                if full || self.dirs.get(&p) != Some(&mt) {
                    self.dirs.insert(p.clone(), mt);
                    new_dirs.push(p);
                }
            }
        }
        for d in new_dirs {
            let Ok(rd) = fs::read_dir(&d) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|x| x.to_str()) != Some("jsonl") || self.files.contains_key(&p) {
                    continue;
                }
                let Ok(m) = e.metadata() else { continue };
                if mtime_ms(&m) < self.started - LOOKBACK_MS {
                    continue;
                }
                self.files.insert(p, Cursor::default());
            }
        }
        let paths: Vec<PathBuf> = self.files.keys().cloned().collect();
        for file in paths {
            let Ok(m) = fs::metadata(&file) else { continue };
            let mt = mtime_ms(&m);
            let cur = self.files.get_mut(&file).unwrap();
            if mt == cur.mtime {
                continue;
            }
            cur.mtime = mt;
            let lines = read_new_lines(&file, cur);
            let id = file.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
            let proj = file.parent().map(Path::to_path_buf).unwrap_or_default();
            let s = self.sessions.entry(id.clone()).or_insert_with(|| Sess {
                id: id.clone(),
                slug: proj.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string(),
                proj_dir: proj,
                cwd: None,
                started: 0.0,
                mtime: 0.0,
                ask: None,
            });
            s.mtime = mt;
            for o in lines {
                self.on_line(&o, &id);
            }
        }
    }

    fn finish(&mut self, id: &str, st: St, ts: f64) {
        if let Some(a) = self.agents.get_mut(id) {
            if a.status == St::Running || a.status == St::Stale {
                a.status = st;
                a.ended = ts;
            }
        }
    }

    fn on_notification(&mut self, text: &str, ts: f64) {
        if !text.contains("<task-notification>") {
            return;
        }
        for block in text.split("</task-notification>") {
            let task = tag(block, "task-id").unwrap_or("");
            let id = tag(block, "tool-use-id").map(str::to_string).or_else(|| self.by_task.get(task).cloned());
            if let Some(id) = id {
                self.finish(&id, map_status(tag(block, "status")), ts);
            }
        }
    }

    fn on_line(&mut self, o: &Value, sid: &str) {
        let ts = o.get("timestamp").and_then(Value::as_str).and_then(parse_ts).unwrap_or_else(now_ms);
        {
            let s = self.sessions.get_mut(sid).unwrap();
            if let Some(c) = o.get("cwd").and_then(Value::as_str) {
                s.cwd = Some(c.to_string());
            }
            if o.get("timestamp").is_some() && (s.started == 0.0 || ts < s.started) {
                s.started = ts;
            }
        }
        let typ = o.get("type").and_then(Value::as_str).unwrap_or("");
        if typ == "queue-operation" {
            if o.get("operation").and_then(Value::as_str) == Some("enqueue") {
                if let Some(t) = o.get("content").and_then(Value::as_str) {
                    self.on_notification(t, ts);
                }
            }
            return;
        }
        if typ != "assistant" && typ != "user" {
            return;
        }
        let Some(content) = o.pointer("/message/content") else { return };
        if let Some(t) = content.as_str() {
            self.on_notification(t, ts);
            return;
        }
        let Some(blocks) = content.as_array() else { return };
        for b in blocks {
            match b.get("type").and_then(Value::as_str).unwrap_or("") {
                "text" if typ == "user" => {
                    if let Some(t) = b.get("text").and_then(Value::as_str) {
                        self.on_notification(t, ts);
                    }
                }
                "tool_use" => {
                    let name = b.get("name").and_then(Value::as_str).unwrap_or("");
                    let id = b.get("id").and_then(Value::as_str).unwrap_or("").to_string();
                    let input = b.get("input").cloned().unwrap_or(Value::Null);
                    match name {
                        "Agent" | "Task" => {
                            let desc = input.get("description").and_then(Value::as_str).unwrap_or("agent");
                            self.agents.insert(
                                id,
                                Agent {
                                    session: sid.to_string(),
                                    agent_id: None,
                                    name: desc.to_string(),
                                    status: St::Running,
                                    started: ts,
                                    ended: 0.0,
                                    last_seen: ts,
                                },
                            );
                        }
                        "AskUserQuestion" => {
                            let q = input
                                .pointer("/questions/0/question")
                                .and_then(Value::as_str)
                                .unwrap_or("Claude a une question.")
                                .to_string();
                            self.sessions.get_mut(sid).unwrap().ask = Some((id, q, ts));
                        }
                        "TaskStop" | "KillShell" | "KillBash" => {
                            let t = input.get("task_id").or_else(|| input.get("shell_id")).and_then(Value::as_str).unwrap_or("");
                            if let Some(r) = self.by_task.get(t).cloned() {
                                self.finish(&r, St::Failed, ts);
                            }
                        }
                        _ => {}
                    }
                }
                "tool_result" => {
                    let id = b.get("tool_use_id").and_then(Value::as_str).unwrap_or("").to_string();
                    let s = self.sessions.get_mut(sid).unwrap();
                    if s.ask.as_ref().is_some_and(|a| a.0 == id) {
                        s.ask = None;
                    }
                    if self.agents.contains_key(&id) {
                        let r = o.get("toolUseResult").filter(|v| v.is_object()).cloned().unwrap_or(Value::Null);
                        if let Some(aid) = r.get("agentId").and_then(Value::as_str) {
                            self.agents.get_mut(&id).unwrap().agent_id = Some(aid.to_string());
                            self.by_task.insert(aid.to_string(), id.clone());
                        }
                        let asyn = r.get("status").and_then(Value::as_str) == Some("async_launched")
                            || r.get("isAsync").and_then(Value::as_bool) == Some(true);
                        if !asyn {
                            let err = b.get("is_error").and_then(Value::as_bool) == Some(true);
                            let st = if err { St::Failed } else { map_status(r.get("status").and_then(Value::as_str)) };
                            self.finish(&id, st, ts);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    /// Agents en arrière-plan : relie l'agent à son journal de sous-agent et
    /// lit le drapeau `finished` de son fichier méta.
    fn scan_agents(&mut self) {
        let now = now_ms();
        let mut meta_maps: HashMap<PathBuf, HashMap<String, String>> = HashMap::new();
        let ids: Vec<String> = self.agents.iter().filter(|(_, a)| a.status == St::Running).map(|(k, _)| k.clone()).collect();
        for id in ids {
            let (sid, aid) = {
                let a = &self.agents[&id];
                (a.session.clone(), a.agent_id.clone())
            };
            let Some(s) = self.sessions.get(&sid) else { continue };
            let dir = s.proj_dir.join(&s.id).join("subagents");
            let aid = match aid {
                Some(a) => a,
                None => {
                    let map = meta_maps.entry(dir.clone()).or_insert_with(|| {
                        let mut m = HashMap::new();
                        if let Ok(rd) = fs::read_dir(&dir) {
                            for e in rd.flatten() {
                                let n = e.file_name().to_string_lossy().to_string();
                                if !n.ends_with(".meta.json") || !n.starts_with("agent-") {
                                    continue;
                                }
                                if let Ok(v) = fs::read(e.path()).map(|b| serde_json::from_slice::<Value>(&b)) {
                                    if let Ok(v) = v {
                                        if let Some(t) = v.get("toolUseId").and_then(Value::as_str) {
                                            m.insert(t.to_string(), n[6..n.len() - 10].to_string());
                                        }
                                    }
                                }
                            }
                        }
                        m
                    });
                    match map.get(&id) {
                        Some(a) => {
                            let a = a.clone();
                            self.agents.get_mut(&id).unwrap().agent_id = Some(a.clone());
                            self.by_task.insert(a.clone(), id.clone());
                            a
                        }
                        None => continue,
                    }
                }
            };
            if let Ok(m) = fs::metadata(dir.join(format!("agent-{aid}.jsonl"))) {
                let a = self.agents.get_mut(&id).unwrap();
                a.last_seen = a.last_seen.max(mtime_ms(&m));
            }
            let finished = fs::read(dir.join(format!("agent-{aid}.meta.json")))
                .ok()
                .and_then(|b| serde_json::from_slice::<Value>(&b).ok())
                .and_then(|v| v.get("finished").and_then(Value::as_bool))
                == Some(true);
            let last = self.agents[&id].last_seen;
            if finished && now - last > 5e3 {
                self.finish(&id, St::Done, last);
            }
        }
    }

    fn mark_stale(&mut self) {
        let now = now_ms();
        for a in self.agents.values_mut() {
            if a.status != St::Running {
                continue;
            }
            let sm = self.sessions.get(&a.session).map(|s| s.mtime).unwrap_or(0.0);
            if now - sm.max(a.last_seen).max(a.started) > STALE_MS {
                a.status = St::Stale;
            }
        }
    }

    /// Questions déposées par le hook Notification. Une question est close dès
    /// que la session écrit de nouveau dans son journal (réponse donnée).
    fn hook_questions(&self) -> HashMap<String, (String, Option<isize>)> {
        let mut out = HashMap::new();
        let Ok(rd) = fs::read_dir(questions_dir()) else { return out };
        for e in rd.flatten() {
            let p = e.path();
            let Some(sid) = p.file_stem().and_then(|s| s.to_str()).map(str::to_string) else { continue };
            let Some(v) = fs::read(&p).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) else { continue };
            let ts = v.get("ts").and_then(Value::as_f64).unwrap_or(0.0);
            let answered = self.sessions.get(&sid).is_some_and(|s| s.mtime > ts + ANSWERED_MARGIN_MS);
            if answered || now_ms() - ts > 6.0 * 3600e3 {
                let _ = fs::remove_file(&p);
                continue;
            }
            let msg = v.get("message").and_then(Value::as_str).unwrap_or("Claude attend ta réponse.");
            let hwnd = v.get("hwnd").and_then(Value::as_i64).map(|h| h as isize);
            out.insert(sid, (msg.to_string(), hwnd));
        }
        out
    }

    pub fn snapshot(&self) -> Model {
        let now = now_ms();
        let hooks = self.hook_questions();
        let mut out: Model = vec![];
        for s in self.sessions.values() {
            let mut agents: Vec<MAgent> = self
                .agents
                .iter()
                .filter(|(_, a)| a.session == s.id)
                .filter(|(_, a)| a.status == St::Running || (a.status != St::Stale && now - a.ended < KEEP_DONE_MS))
                .map(|(id, a)| MAgent { id: id.clone(), name: a.name.clone(), status: a.status, started: a.started })
                .collect();
            let focus = hooks.get(&s.id).and_then(|q| q.1);
            let question = hooks.get(&s.id).map(|q| q.0.clone()).or_else(|| {
                s.ask.as_ref().filter(|a| now - a.2 > ASK_DELAY_MS).map(|a| a.1.clone())
            });
            if agents.is_empty() && question.is_none() && now - s.mtime > SESSION_ACTIVE_MS {
                continue;
            }
            agents.sort_by(|a, b| a.started.total_cmp(&b.started));
            let name = s
                .cwd
                .as_deref()
                .and_then(|c| Path::new(c).file_name())
                .and_then(|n| n.to_str())
                .map(str::to_string)
                .unwrap_or_else(|| s.slug.clone());
            out.push(MSession {
                id: s.id.clone(),
                name,
                started: s.started,
                busy: now - s.mtime < SESSION_BUSY_MS,
                question,
                focus,
                agents,
            });
        }
        out.sort_by(|a, b| a.started.total_cmp(&b.started));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Affiche ce que le widget voit des vrais journaux : cargo test dump -- --ignored --nocapture
    #[test]
    #[ignore]
    fn dump() {
        let mut w = Watcher::new();
        w.poll();
        for s in w.snapshot() {
            println!("{} [{}] busy={} question={:?}", s.name, &s.id[..8.min(s.id.len())], s.busy, s.question);
            for a in s.agents {
                println!("    {:?} {}", a.status, a.name);
            }
        }
    }

    #[test]
    fn ts() {
        assert_eq!(parse_ts("1970-01-01T00:00:01.500Z"), Some(1500.0));
        assert_eq!(parse_ts("2000-03-01T00:00:00Z"), Some(951868800000.0));
    }
}
