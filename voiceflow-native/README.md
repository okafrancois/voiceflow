# VoiceFlow Native — réécriture macOS (Swift/SwiftUI)

Réécriture native de VoiceFlow, ciblant macOS 26+ (Apple Silicon), Liquid Glass.

- `index.html` — maquette de design (bureau macOS simulé) : `python3 -m http.server 5849`
- `VoiceFlow/` — package SwiftPM de l'application
- `build.sh` — compile et assemble `dist/VoiceFlow.app` (signature ad hoc)

## Phase 1 — squelette de bout en bout (état actuel)

Une seule fonctionnalité, mais réelle : **maintenir ⌥ Espace → dicter → relâcher →
le texte s'insère dans l'app active.**

Chaîne : `HotkeyManager` (CGEventTap, avale ⌥ Espace) → `AudioRecorder`
(AVAudioEngine) → `TranscriptionSession` (SpeechAnalyzer/SpeechTranscriber,
100 % sur l'appareil) → `TextInjector` (port fidèle de
`apps/desktop/src-tauri/src/text_injector/macos.rs` : cible AX capturée au
démarrage, sinon frappe simulée ≤ 400 graphèmes, sinon presse-papiers + Cmd+V
avec sauvegarde/restauration intégrale).

### Lancer

```sh
./build.sh && open dist/VoiceFlow.app
```

Au premier lancement :
1. accorder le **micro** (boîte système) ;
2. accorder l'**accessibilité** (invite système → Réglages > Confidentialité) ;
3. relancer l'app après l'accord accessibilité (le CGEventTap est créé au lancement) ;
4. le modèle de langue se télécharge en arrière-plan (icône ↓ dans la barre de menus).

La signature ad hoc change à chaque build : macOS peut redemander la case
Accessibilité après recompilation (décocher/recocher dans les Réglages).

### À valider (objectifs de la phase)

- [ ] Qualité du **français** de SpeechAnalyzer vs l'app Tauri actuelle
      (mêmes dictées, comparer). Plan B si décevant : WhisperKit.
- [ ] Latence fin de dictée → texte inséré.
- [ ] Parité d'injection : Cursor, Mail, Safari, Terminal, champ Spotlight.
      Vérifier le mode AX (insertion sans réactivation) et la restauration
      du presse-papiers.

### Interface

Calquée sur l'app Tauri, mesurée sur une capture de l'app réelle
(`/Applications/Voice Flow.app`) plutôt que devinée :

- fenêtre sans barre de titre, barre latérale sombre de 248 px pleine hauteur,
  feux tricolores au-dessus du logo ;
- logo + « Voice Flow » en serif italique 22, filet de séparation, navigation
  en pastilles pleinement arrondies (sélection = fond `card` + bordure) ;
- contenu à 40 px de marge, largeur max 1000 : titre 28 semibold + sous-titre
  gris, bande d'état bordée à 24 px de rayon, cartes à 18 px ;
- cartes de métrique : libellé 13 gris **au-dessus**, valeur 36 en dessous ;
- listes en cartes bordées avec filets internes, lignes dépliables au clic.

Palette de `src/index.css` : `#F9F9F9`/`#FFFFFF`/`#EBEBEB` en clair,
`#1B1B1B`/`#212121`/`#343434` en sombre.

### Implémenté

- **Dictée** : SpeechAnalyzer (Apple) ou Whisper (tiny/base/small/large-v3),
  langue indépendante du système, détection automatique côté Whisper.
  Téléchargement des modèles visible, avec progression et suppression.
- **Entrée audio** : choix du micro (CoreAudio), réduction de bruit par le
  traitement vocal du système, coupe du silence réglable.
- **Un seul raccourci**, personnalisable, touche seule acceptée (Fn, F1–F20) ;
  trois modes — maintenir, basculer, double appui. Le polissage est un
  interrupteur, pas un second raccourci.
- **Insertion** : champ d'origine mémorisé au déclenchement (désactivable),
  sinon frappe simulée ou presse-papiers selon la longueur.
- **Polissage** : Apple Intelligence sur l'appareil. Six styles, prompts
  système visibles et modifiables, retour possible à l'original, règles par
  application.
- **Dictionnaire** : saisie manuelle, variantes « aussi entendu », import CSV,
  et apprentissage des corrections faites après insertion.
- **Extraits**, **historique SQLite**, **statistiques** 7/30 jours et tout
  l'historique, rétention configurable.
- **Apparence** : thème clair/sombre/système ; pill configurable en position
  (six ancrages ou libre), taille, couleur, opacité et mode d'affichage.
- **Langue de l'interface** : français et anglais (`Resources/*.lproj`),
  appliquée au lancement suivant.
- **Onboarding** au premier lancement, **mises à jour** par flux JSON.
- **Confort** : ouverture à la connexion, sons repris de l'app Tauri, icône
  générée.

### Choix assumé : Whisper plutôt que MLX

Faire tourner les modèles locaux de l'app Tauri (Qwen, Gemma…) via MLX Swift
est impossible **en même temps que WhisperKit** : les deux dépendent de
`swift-transformers` dans des versions disjointes (WhisperKit ≤ 1.2,
mlx-swift-examples ≥ 1.3 sur `main`). Whisper est conservé pour la
transcription ; le polissage reste sur le modèle du système. Le moteur MLX
écrit et vérifié attend dans `attente/`, avec la marche à suivre.

Note d'outillage : compiler MLX exige la chaîne Metal, installée depuis
(`xcodebuild -downloadComponent MetalToolchain`, 839 Mo).

### Reste ouvert

- Pont éditeur « vibe coding » et services cloud : écartés volontairement.
- Conservation de l'audio (et donc relecture, retranscription, traduction
  depuis l'historique) : écartée volontairement.
- Streaming du texte poli (le résultat arrive d'un bloc).
- L'installation automatique des mises à jour demanderait Sparkle et une paire
  de clés ; aujourd'hui l'app signale la version et ouvre le lien.
