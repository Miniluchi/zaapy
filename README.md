# Zaapy

**Voyage automatique dans Dofus 3 depuis les guides Ganymède : cliquez une
position, votre personnage s'y rend. Sans copier-coller, sans changer de
fenêtre.**

<p align="center">
  <img src="assets/icon-1024.png" width="128" alt="Icône Zaapy – voyage automatique Dofus 3">
</p>

## Ce que ça fait

Cliquer une coordonnée dans un guide Ganymède copie `/travel x,y` dans votre
presse-papiers. Normalement, il faut ensuite passer sur Dofus, ouvrir le chat,
coller, appuyer sur Entrée, puis revenir au guide.

Zaapy s'en charge : dès que Ganymède copie une commande de voyage, il met Dofus
au premier plan et tape la commande dans le chat. Vous ne quittez jamais le
guide.

Fonctionne sur **Windows** et **macOS**.

## Installation

Téléchargez la dernière version sur la
[page Releases](https://github.com/Miniluchi/zaapy/releases) :

| Système | Fichier                                               |
| ------- | ----------------------------------------------------- |
| Windows | `Zaapy_1.3.0_x64-setup.exe`                           |
| macOS   | `Zaapy_1.3.0_universal.dmg` (Intel et Apple Silicon)  |

Zaapy n'est pas signé — les certificats coûtent plus cher chaque année que ce
projet gratuit ne rapporte — donc les deux systèmes vous avertiront au premier
lancement.

### Windows

1. Lancez l'installateur. Windows affiche un écran bleu **« Windows a protégé
   votre ordinateur »**.
2. Cliquez sur **Informations complémentaires**, puis **Exécuter quand même**.
3. Pas besoin de droits administrateur : l'installation est faite pour vous
   seul, et le runtime WebView2 est téléchargé automatiquement s'il manque.

### macOS

1. Ouvrez le `.dmg` et glissez Zaapy dans **Applications**.
2. **Ne double-cliquez pas dessus.** Faites un clic droit sur Zaapy dans
   Applications, choisissez **Ouvrir**, puis **Ouvrir** à nouveau dans la
   fenêtre de dialogue. Une seule fois suffit.
3. Zaapy a besoin d'une autorisation pour contrôler les autres fenêtres. Ouvrez
   ses réglages depuis l'icône de la barre des menus, cliquez sur **Grant
   permission…**, puis acceptez dans la fenêtre affichée par macOS.
4. Si aucune fenêtre n'apparaît — macOS ne demande qu'une fois — activez Zaapy à
   la main dans **Réglages Système → Confidentialité et sécurité →
   Accessibilité**.

Sans cette autorisation, Zaapy ne peut ni afficher votre fenêtre Dofus ni y
écrire, et ses réglages vous le signaleront au lieu d'échouer en silence.

## Utilisation

Suivez votre guide comme d'habitude et cliquez une position. Zaapy prend le
relais. Les flèches de l'outil de chasse au trésor de Ganymède fonctionnent de
la même façon.

Cliquer un objet dans Ganymède colle son nom dans Dofus — dans la recherche de
l'hôtel des ventes, par exemple — sans ouvrir le chat. Désactivez-le avec
**Items** dans les réglages.

Par défaut, le chat de Dofus garde le clavier après une commande. Cochez
**Press Escape** dans les réglages pour le rendre au jeu.

Les commandes `/zaap`, y compris les `/travel` enchaînés, sont relayées de la
même façon. Désactivez-les avec **/zaap** dans les réglages.

Quand une nouvelle version sort, Zaapy le signale au démarrage et ajoute
**Update to…** au menu de son icône (zone de notification ou barre des menus).
Rien n'est installé tant que vous n'avez pas cliqué.

## Confidentialité

- Zaapy **lit seulement** votre presse-papiers, sans jamais le vider ni le
  modifier — après un envoi, la commande y est toujours pour un collage manuel.
- Il ne réagit qu'à ce que vous copiez **dans Ganymède** ; tout le reste est
  ignoré.
- Il n'écrit jamais dans une fenêtre sans avoir vérifié qu'elle est au premier
  plan.
- Rien n'est envoyé nulle part : pas de compte, pas de serveur, pas de
  télémétrie. La seule requête de Zaapy est la vérification des mises à jour au
  démarrage, auprès de GitHub.

## Licence et code source

Zaapy est gratuit et open source, sous [licence MIT](LICENSE).

- [Contribuer et compiler depuis les sources](docs/DEVELOPMENT.md)
- [Ce qu'est le produit, et ce qu'il n'est volontairement pas](docs/VISION.md)
- [Comment il est construit](docs/ARCHITECTURE.md)
