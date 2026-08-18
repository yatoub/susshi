# ── Fenêtre erreur ───────────────────────────────────────────────────────────
error-title = ⚠  Erreur
error-dismiss = Appuyez sur Entrée ou Esc pour fermer

# ── Onglets connexion ─────────────────────────────────────────────────────────
tab-title = Mode de Connexion (Tab pour changer)
tab-direct = Direct [1]
tab-jump = Rebond [2]
tab-wallix = Wallix [3]

# ── Toggle verbose ────────────────────────────────────────────────────────────
verbose-title = Options (v pour basculer)
verbose-label = Verbose (-v)

# ── Barre de recherche ────────────────────────────────────────────────────────
search-idle-hint = Appuyez sur / pour rechercher…
search-title-idle = Recherche (/)
search-placeholder = (nom ou hôte, Échap pour annuler)
search-title-active = 🔍 Recherche nom/hôte ({ $total } serveurs)
search-no-results = 🔍 Aucun résultat pour '{ $query }'
search-all-match = 🔍 { $count } serveurs correspondent
search-partial = 🔍 { $found } / { $total } serveurs
search-result-all = ✓ { $count } serveurs affichés
search-result-partial = ✓ { $found } / { $total } correspondent à '{ $query }'

# ── Panneaux principaux ───────────────────────────────────────────────────────
panel-servers = Serveurs
panel-details = Détails
details-placeholder = Sélectionnez un serveur pour voir les détails.
details-namespace = 📦 Namespace : { $label }
details-group = Group: { $name }
details-environment = Environment: { $group } / { $env }

# ── Libellés du panneau détails ───────────────────────────────────────────────
label-name = Nom:
label-host = Hôte:
label-port = Port:
label-user = Util.:
label-mode = Mode:
label-key = Clé:
label-jump = Rebond:
label-wallix = Wallix:
label-options = Options:

# ── Bloc diagnostic ───────────────────────────────────────────────────────────
probe-section = ─── Système ─────────────────────
probe-hint =   d — diagnostiquer
probe-running = Diagnostic en cours…
probe-kernel = Kernel
probe-cpu = CPU
probe-cpu-cores = Cœurs
probe-os = OS
probe-load = Charge
probe-ram = RAM
probe-disk = Disk /
probe-wallix-error = Diagnostic non disponible en mode Wallix
probe-disk-extra = Disk { $mount }
probe-fs-absent = ⚠  { $mount } — non monté

# ── Barre de statut ───────────────────────────────────────────────────────────
status-normal = Navigation : ↑/↓ | Ouvrir : Espace/Entrée | Recherche : / | Mode : Tab/1-3 | v : Verbose | y : Copier | d : Probe | f : Favori | F : Vue favs | r : Recharger | x : Cmd | H : Tri | C : Replier tout | q : Quitter
status-searching = Recherche : Tapez pour filtrer… | Échap : Annuler | Ctrl+U : Effacer | Entrée : Valider
status-search-active = Navigation : ↑/↓ | Effacer : Échap | Nouvelle recherche : / | Verbose : v | Entrée : Connecter | q : Quitter

# ── Aides clavier ─────────────────────────────────────────────────────────────
hint-navigate = naviguer
hint-validate-cancel = valider / annuler
hint-clear = effacer
hint-connect = connexion
hint-clear-filter = effacer filtre
hint-new-search = nouvelle recherche
hint-quit = quitter
hint-expand = expand
hint-search = recherche
hint-mode = mode
hint-tunnels = tunnels
hint-probe = probe
hint-command = commande
hint-scp = SCP
hint-copy-ssh = copier SSH
hint-favorite = favori
hint-favorites-view = ★ vue favoris
hint-reload = recharger
hint-recent-sort = tri récent
hint-collapse = replier
hint-expand-all = tout déplier
hint-verbose = verbose
hint-theme-toggle = thème suivant
hint-mouse-toggle = sélection texte
mouse-capture-off = [souris désactivée — sélection texte active]

# ── Overlay sélecteur Wallix ──────────────────────────────────────────────────
wallix-selector-title = Sélection Wallix
wallix-selector-loading = Chargement des entrées Wallix pour { $server }…
wallix-selector-loading-hint = Connexion au bastion et lecture du menu interactif.
wallix-selector-cancel-hint = Esc/q : annuler
wallix-selector-error = Erreur du sélecteur Wallix pour { $server }
wallix-selector-close-hint = Entrée/Esc/q : fermer
wallix-selector-choose = Sélectionne l'entrée Wallix pour { $server } ({ $host })
wallix-selector-list-hint = ↑/↓ : naviguer | Entrée : connecter | Esc/q : annuler

# ── Avertissements includes ───────────────────────────────────────────────────
include-warn-load = Impossible de charger '{ $label }' ({ $path }) : { $error }
include-warn-circular = Dépendance circulaire ignorée : '{ $label }' ({ $path })
include-warn-nested = Les includes imbriqués dans '{ $label }' sont ignorés (v0.7)
include-warn-git-outdated = Le dépôt git '{ $path }' a { $behind } commit(s) de retard sur sa branche amont — pensez à faire un git pull.

# ── Messages de statut ────────────────────────────────────────────────────────
copied = Copié : { $cmd }
clipboard-error = Erreur presse-papiers : { $error }
clipboard-unavailable = Presse-papiers indisponible
ssh-error = Erreur SSH : { $error }

# ── Historique des connexions ─────────────────────────────────────────────────
last-seen-label = Dern. conn.:
last-seen-never = —
last-seen-ago = il y a { $duration }
last-seen-just-now = à l'instant

# ── Rechargement à chaud ──────────────────────────────────────────────────────
config-reloaded = Config rechargée ({ $count } serveurs)
config-reload-error = Erreur rechargement config

# ── Favoris ───────────────────────────────────────────────────────────────────
favorites-title = ⭐ Favoris
favorite-added = ⭐ Ajouté aux favoris
favorite-removed = Favori retiré

# ── Sort par récence ──────────────────────────────────────────────────────────
sort-recent-on = Tri : récent  [H]
sort-recent-off = Tri : alpha   [H]

# ── Commande ad-hoc ───────────────────────────────────────────────────────────
cmd-prompt = Commande :
cmd-running = Exécution…
cmd-exit-err = Erreur (exit { $code })

# ── Validation YAML ───────────────────────────────────────────────────────────
validation-title = ⚠  Avertissements de configuration
validation-unknown-field = { $file } ({ $context }): champ inconnu « { $field } »

# ── Tunnels SSH ───────────────────────────────────────────────────────────────
tunnel-wallix-unavailable = Tunnels SSH non disponibles en mode Wallix
tunnel-not-found = Tunnel #{ $index } introuvable pour ce serveur
tunnel-already-active = Tunnel « { $label } » déjà actif (port { $port })
tunnel-started = Tunnel « { $label } » démarré sur le port { $port }
tunnel-error = Erreur tunnel : { $error }
tunnel-stopped = Tunnel « { $label } » (port { $port }) arrêté
tunnel-died = Tunnel « { $label } » (port { $port }) s'est arrêté : { $reason }
tunnel-deleted = Tunnel supprimé
tunnel-updated = Tunnel mis à jour
tunnel-added = Tunnel ajouté
tunnel-overlay-new = + (nouveau tunnel)
tunnel-overlay-hints1 =   ↑↓ naviguer   Enter démarrer/arrêter   Del supprimer
tunnel-overlay-hints2 =   e éditer      a ajouter                q/Esc fermer
tunnel-form-edit-title = Modifier le tunnel — { $server }
tunnel-form-new-title = Nouveau tunnel — { $server }
tunnel-form-field-label =   Label        :
tunnel-form-field-local-port =   Port local   :
tunnel-form-field-remote-host =   Hôte distant :
tunnel-form-field-remote-port =   Port distant :
tunnel-form-hint =   Tab champ suivant   Enter valider   Esc annuler
tunnel-form-local-port-invalid = Port local invalide (entier 1–65535 attendu)
tunnel-form-remote-host-empty = Hôte distant obligatoire
tunnel-form-remote-port-invalid = Port distant invalide (entier 1–65535 attendu)
tunnel-badge-label = Tunnels :
tunnel-badge-active =
    { $n_run ->
        [one]   { $n_run } actif / { $n_cfg ->
            [one]  { $n_cfg } configuré
           *[other] { $n_cfg } configurés
        }
       *[other] { $n_run } actifs / { $n_cfg ->
            [one]  { $n_cfg } configuré
           *[other] { $n_cfg } configurés
        }
    }
tunnel-badge-none =
    { $n_cfg ->
        [one]  { $n_cfg } configuré, aucun actif
       *[other] { $n_cfg } configurés, aucun actif
    }

# ── SCP ───────────────────────────────────────────────────────────────────────
scp-wallix-unavailable = SCP non disponible en mode Wallix
scp-done-ok = SCP terminé ✔
scp-done-err = SCP terminé avec des erreurs ✗
scp-failed = SCP échoué : { $error }
scp-form-local-required = Le chemin local est obligatoire
scp-form-remote-required = Le chemin distant est obligatoire
scp-direction-title = Transfert SCP — { $server }
scp-direction-upload-label = Envoi
scp-direction-download-label = Téléchargement
scp-direction-upload = (local → serveur)
scp-direction-download = (serveur → local)
scp-direction-hint =   Esc annuler
scp-form-title = SCP { $direction } — { $server }
scp-form-field-local =   Local   :
scp-form-field-remote =   Distant :
scp-form-hint =   Tab changer de champ   Enter confirmer   Esc annuler
scp-result-title = Résultat SCP
scp-result-success = SCP { $direction } terminé avec succès
scp-result-errors = SCP { $direction } terminé avec des erreurs
scp-result-fail = Erreur SCP : { $error }
scp-result-hint =   Enter / Esc  fermer
scp-in-progress = SCP { $direction } en cours...
scp-eta-label = Restant

# ── Saisie de credential ──────────────────────────────────────────────────────
credential-input-title-passphrase = Passphrase clé SSH — { $server }
credential-input-title-password = Mot de passe SSH — { $server }
credential-input-prompt-passphrase =   Passphrase :
credential-input-prompt-password =   Mot de passe :
credential-input-hint =   Enter confirmer   Esc annuler
probe-cm-label = ControlMaster  
probe-cm-active = actif
probe-cm-inactive = inactif

# ── Overlay aide clavier ──────────────────────────────────────────────────────
help-title = Aide — raccourcis clavier  (h / Esc pour fermer)
help-navigate-down = Descendre
help-navigate-up = Monter
help-connect = Connecter
help-change-mode = Changer de mode (Direct / Jump / Wallix)
help-select-mode = Sélectionner le mode directement
help-search = Rechercher
help-close = Quitter la recherche / fermer
help-quit = Quitter susshi
help-expand-group = Développer / réduire un groupe
help-favorite = Ajouter / retirer des favoris
help-favorites-view = Afficher uniquement les favoris
help-recent-sort = Trier par dernière connexion
help-collapse-all = Réduire tous les groupes
help-expand-all = Déplier tous les groupes
help-theme-toggle = Basculer le thème Catppuccin (Latte→Frappe→Macchiato→Mocha)
help-mouse-toggle = Désactiver la capture souris (sélection texte native)
help-reload = Recharger la configuration
help-verbose = Mode verbeux SSH
help-copy-ssh = Copier la commande SSH
help-credential = Saisir credential (passphrase / mot de passe)
help-command = Exécuter une commande ad-hoc (↑/↓ pour l'historique)
help-tunnels = Gérer les tunnels SSH
help-scp = Transfert SCP
help-probe = Diagnostic SSH (probe)
help-overview = Dashboard overview du groupe sélectionné
help-pin = Épingler / dés-épingler le serveur (split pane)
help-help = Afficher / masquer cette aide
help-goto-top-bottom = Aller en haut / bas de la liste
help-page = Page précédente / suivante

# ── Wizard de première configuration ────────────────────────────────────────
wizard-title = 👋 Bienvenue — ajoutons votre premier serveur
wizard-field-group = Nom du groupe :
wizard-field-server = Nom du serveur :
wizard-field-host = Hôte :
wizard-field-user = Utilisateur (optionnel) :
wizard-hint = Tab : champ suivant · Entrée : créer · Échap : ignorer (config vide)
wizard-error-group-name = Le nom du groupe est requis
wizard-error-server-name = Le nom du serveur est requis
wizard-error-host = L'hôte est requis
wizard-created = ~/.susshi.yml créé — bienvenue !
