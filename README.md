# PokAgent

Tes sessions et agents Claude Code en pixel art, posés sur ton fond d'écran.

- **Quart haut-gauche de l'écran principal** : une salle par session (Claude), puis une salle par agent. 12 salles au plus ; au-delà, une étiquette « +N hors cadre ».
- **Quart bas-droit** : quand une session attend ta réponse, son personnage apparaît avec la question et un petit jingle. Un clic ferme la bulle.
- **Trois univers** : Labo (dresseurs, style DS), Village (île, habitants animaux), Donjon (chevaliers et torches). Le statut est toujours en couleur : bleu en cours, or terminé, rouge échec.

Le widget reste sous toutes tes fenêtres, juste au-dessus du fond d'écran, et ne capte aucun clic.

## Installer

1. Télécharge `pokagent.exe` depuis la [dernière release](https://github.com/williamloree/PokAgent/releases/latest).
2. Range-le où tu veux le garder (par exemple `%LOCALAPPDATA%\Programs\PokAgent\`), puis lance-le.

Au premier lancement, PokAgent s'inscrit pour démarrer avec Windows. Tout se règle depuis l'icône de la zone de notification : univers, images par seconde (2, 8 ou 20), son, hook des questions, lancement avec Windows, mises à jour, quitter. Les réglages sont dans `%APPDATA%\PokAgent\config.json`.

```
pokagent.exe                 # lit les journaux de Claude Code
pokagent.exe --demo          # données fictives, pour essayer
pokagent.exe update          # cherche et installe une mise à jour
pokagent.exe install-hook    # ajoute le hook des questions à Claude Code
```

## Mises à jour

Une minute après le lancement, puis toutes les 6 heures, PokAgent regarde la dernière release de ce dépôt. Si elle est plus récente, il télécharge `pokagent.exe`, vérifie son empreinte SHA-256 (`pokagent.exe.sha256`), remplace son propre exécutable et redémarre. Désactivable dans le menu de l'icône.

Pour publier une version : augmenter `version` dans `Cargo.toml` et pousser sur `main`. Le workflow `.github/workflows/build.yml` compile, teste et crée la release `vX.Y.Z` si elle n'existe pas encore.

## Questions de Claude

Sans rien installer, PokAgent repère les questions posées avec `AskUserQuestion`. Pour être prévenu aussi des demandes de permission, installe le hook `Notification` de Claude Code : menu de l'icône › « Installer le hook des questions… », ou `pokagent.exe install-hook`. Il ajoute une entrée dans `%USERPROFILE%\.claude\settings.json` (copie de l'ancien fichier dans `settings.json.bak`).

Avec le hook, un clic sur la bulle ramène au premier plan la fenêtre où tourne la session (Windows Terminal, VS Code…). Sans hook, le clic ferme seulement la bulle.

## Compiler

```
cargo build --release        # target\release\pokagent.exe
cargo test
```

## Ce que ça coûte

Mesuré en mode démo à 8 images/s sur un écran 1920×1080 : environ 1 % d'un cœur et 6 Mo de mémoire privée. Le dessin se fait sur le CPU en 480×270, agrandi ×2 ; seuls les pixels qui changent sont renvoyés à Windows. Rien n'est dessiné quand une application plein écran est au premier plan.

## Code

| Fichier | Rôle |
|---|---|
| `src/watcher.rs` | Lit `~/.claude/projects/**/*.jsonl` (portage du watcher de ClaudeIsles), une fois par seconde, dans son propre fil. |
| `src/hook.rs` | Sous-commandes `hook` (appelée par Claude Code) et `install-hook`. |
| `src/updater.rs` | Mise à jour depuis les releases GitHub (WinHTTP, vérification SHA-256). |
| `src/autostart.rs` | Lancement avec Windows (clé `Run` du registre utilisateur). |
| `src/scene.rs` | Salles, personnages qui se promènent, popup de question, mode démo. |
| `src/themes.rs` | Les trois univers : décor, indicateur de statut, porte, personnages. |
| `src/sprites.rs` | Personnages 16×24 (humains et animaux), dessinés une fois par agent. |
| `src/gfx.rs` | Tampon de pixels et primitives de dessin. |
| `src/main.rs` | Fenêtres Windows transparentes, icône de notification, son, boucle. |
