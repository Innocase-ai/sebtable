# Sebtable

Base de données locale **type Airtable** : workspace multi-bases SQLite, grille éditée en ligne, relations, formules, IA hybride — le tout **100 % local**, sans compte ni cloud.

Construit avec **Tauri v2** (Rust + SQLite) et **React 18 + TypeScript** (Vite).

---

## Fonctionnalités

- **Workspace multi-bases** : une ou plusieurs bases SQLite, avec référentiels partagés (relations cross-DB) et recherche plein-texte sur tout le workspace.
- **Grille type Airtable** : édition inline, filtres, tris, groupes, pagination, vues, colonnes calculées, champs de type text/number/checkbox/select/email/url/phone/date/link/attachment…
- **Relations** : champs *Link* (many-to-many, backlinks auto), *Lookup*, *Rollup*, *Count* — intra-base **et** cross-base.
- **Formules** : parseur + évaluateur Rust (IF, SWITCH, CONCATENATE, SUM/AVERAGE/MIN/MAX, LEFT/RIGHT/MID, REGEX_MATCH, DATETIME_DIFF/FORMAT, ARRAYJOIN/UNIQUE…) — documentation interactive dans `docs/formules`.
- **IA hybride** : LM Studio (local, OpenAI-compatible) **ou** OpenAI, avec repli **heuristique offline** (aucun appel réseau si aucun provider n'est configuré) :
  - suggestion de relations, génération de formules en langage naturel,
  - analyse de table (stats, insights), assistant de nettoyage (trim, déduplication, normalisation…).
- **Import / Export** : CSV, JSON, XLSX (limites : 20 Mo, 10 000 lignes).
- **Pièces jointes** : champ *attachment* avec upload, prévisualisation et suppression.
- **Sécurité** : clé OpenAI stockée dans le keychain du système (jamais en clair), CSP Tauri, scope `shell:open` restreint aux URLs http(s), chemins de fichiers validés/canonicalisés.

## Stack technique

| Couche | Choix |
|---|---|
| Desktop | Tauri v2 (Rust) |
| Base de données | SQLite via `sqlx` (migrations versionnées, une connexion) |
| IA | `reqwest` + providers LM Studio / OpenAI, heuristiques Rust |
| Frontend | React 18, TypeScript, Vite |
| État / données | Zustand, TanStack Table, TanStack Query, react-virtual |
| Packaging | pnpm, GitHub Actions (`quality.yml` : test + build Windows) |

## Prérequis

- Node.js ≥ 18 + pnpm
- Rust stable (toolchain MSVC sous Windows) + VS Build Tools
- (Tauri) les dépendances natives de la plateforme — cf. [Tauri prerequisites](https://tauri.app/start/prerequisites/)

## Lancer Sebtable

### 1. En 1 clic (Windows, recommandé pour tester)

Double-clic sur **`lancer-sebtable.bat`** à la racine du projet :
- si Rust est installé → lance l'app complète (`pnpm tauri dev` : fenêtre Tauri + SQLite)
- sinon → fallback web seul (`pnpm dev` sur http://localhost:1420)

Variantes : `lancer-sebtable.ps1` (PowerShell) et `lancer-sebtable-debug.bat` (logs détaillés).

### 2. Mode développement (contributeurs)

```bash
pnpm install        # installer les dépendances front
pnpm tauri dev      # compile Rust + sert le frontend (HMR)
```

### 3. Binaire d'installation

```bash
pnpm tauri build    # produit l'installateur Windows (.msi/.exe) dans src-tauri/target/release/bundle/
```

## Vérification

```bash
cargo test          # dans src-tauri/ — 31 tests (unitaires + intégration)
cargo clippy        # aucun nouveau warning
pnpm build          # tsc + vite build
```

## Structure

```
src-tauri/
  migrations/            # schéma des bases (meta : tables/champs/relations/vues)
  src/
    db/                  # repository SQL, modèles, connexion
    formula/             # parser + évaluateur de formules
    ai/                  # providers IA + heuristiques (suggest/formule/analyse/nettoyage)
    commands/            # commandes Tauri (tables, vues, workspace, IA, import/export, pièces jointes)
    workspace/           # gestion du workspace, migrations
src/
  components/            # grille, modals, IA, import/export, settings, workspace…
  hooks/  stores/  lib/  # data fetching, état global, couche API
docs/formules/           # page interactive de documentation des formules
```

## Roadmap

- [x] Fondations, grille, relations intra-DB
- [x] Filtres corrects, sécurité & intégrité des données
- [x] UX / accessibilité, performance
- [x] Cross-DB / référentiels
- [x] IA hybride (MVP heuristique + LLM)
- [x] Import/export, pièces jointes, paramètres, packaging
- [ ] Champs visibles (`visible_field_ids`), nettoyage de dette technique
