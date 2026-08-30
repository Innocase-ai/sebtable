# Sebtable — Suivi de progression

> **Règle de session** : charger ce fichier à chaque nouveau run pour reprendre le contexte (todo `todowrite` correspondant aux sections "À faire").

## Projet
Clone local Airtable : Tauri v2 + React 18 + TypeScript + Vite + SQLite (sqlx) + Zustand + TanStack Table.
Spécification : `C:\Users\sebas\Downloads\SPEC.md`.

## Etat global
- **Phase 0 (fondations)** : ✅ terminée
- **Phase 1 (grille)** : ✅ terminée
- **Phase 2 (relations intra-DB)** : ✅ terminée (back + front)
- **Audit complet** : ✅ fait (rapports détaillés en session)
- **Sprint 1 (sécurité/données)** : ✅ terminé
- **Sprint 2 (filtres)** : ✅ terminé
- **Audit bugs (5)** : ✅ terminé
- **Race schéma SQLite (tests)** : ✅ terminé
- **Sprint 3 (UX/A11y)** : ✅ terminé
- **Sprint 4 (performance)** : ✅ terminé
- **Phase 3 (cross-DB/référentiels)** : ✅ terminée (MVP)
- **Phase 4 (IA hybride)** : ✅ terminée (MVP heuristique + LLM hybrid)
- **Phase 5 (Polish & Avancé)** : ✅ terminée (MVP)

---

## ✅ FAIT

### Phase 0 — Fondations
- Toolchain vérifiée (Rust MSVC, VS Build Tools, pnpm/Node)
- `WorkspaceManager` : create/open/switch db, `workspace_index` (all DBs)
- Migrations sqlx : `_tables`, `_fields`, `_relations`, `_views`
- Commandes workspace enregistrées dans `lib.rs`

### Phase 1 — Grille
- CRUD tables/fields/views complet
- `get_table_data` : filtres/sorts/groupes/pagination (QueryBuilder)
- `upsert_records` / `delete_records`
- Frontend : DataGrid (TanStack + react-virtual), FilterToolbar, Cell inline-edit, WorkspaceLauncher, Sidebar, DatabaseSwitcher, modals, stores (workspace/table/ui), hooks, api layer

### Phase 2 — Relations intra-DB
- **Backend** :
  - `models.rs` : `LinkValue`, `LinkTarget`, configs (Link/Lookup/Rollup/Count/Formula), `Field::is_stored()`, `is_backlink()`, `link_config()`, `formula_config()`
  - `formula/` : parser récursif + évaluateur Rust (IF, SWITCH, AND/OR/NOT, CONCATENATE, LEFT/RIGHT/MID, REGEX_MATCH/EXTRACT, SUM/AVERAGE/MIN/MAX, ROUND, DATETIME_DIFF/FORMAT/DATEADD, ARRAYJOIN/UNIQUE/COMPACT…)
  - `repository.rs` : `create_link_field` (forward+backlink+`_relations`), `link_records`/`unlink_records`, `resolve_link_displays` (batch), `compute_computed_fields`, `compute_formula_field`, cascade delete
  - Commandes : `create_link_field`, `link_records`, `unlink_records`
- **Frontend** :
  - `types/field.ts` : LinkValue, configs, `isStoredField()`, `isLinkField()`, `isBacklink()`
  - `api.ts` : `createLinkField`, `linkRecords`, `unlinkRecords`
  - `LinkCell.tsx` (chips) + `LinkPickerModal.tsx` (sélection multi/radio)
  - `CreateFieldModal.tsx` étendu (config par type)
  - Sort/filter limités aux champs stockés, icône ƒ pour calculés, cellules read-only

### Sprint 2 — Filtres corrects (backend `repository.rs`)
- **C4** : `is_empty` checkbox → `IS NULL OR = 0` (non coché = vide) ✅
- **E10** : `is_not_empty` number → `IS NOT NULL` (0 inclus) ; checkbox → `= 1` ✅
- **S2** : `escape_like()` échappe `%`/`_`/`\` + clause `ESCAPE '\'` sur contains/does_not_contain ✅
- **E8** : `push_value` nombre invalide → `NULL` (aucune correspondance) au lieu de `0.0` ✅
- Test `filters_correctness` (S2/E10/C4/E8 couverts) ✅

### Sprint 1 — Sécurité & intégrité données
- **C5** : `format_date_safe()` — validation StrftimeItems avant `dt.format()` (anti-panic)
- **S1** : `compile_regex()` — `size_limit` + `dfa_size_limit` (anti-DoS regex)
- **C1** : `delete_records` transactionnel + `cleanup_dangling_links()` (nettoyage liens JSON orphelins)
- **C2/C3** : `link_records`/`unlink_records` transactionnels + validation champ lien; `read/write_link_value` généralisés (`Executor`)

### Audit (revue du code) — 5 bugs corrigés
- **Critique — perte de données à l'édition de cellule** : `upsert_records` construisait
  `DO UPDATE SET <tous les champs>` en bindant `NULL` pour les champs absents → écrasait
  silencieusement toutes les autres colonnes. Fix : ne mettre à jour que les champs présents
  dans le payload (`obj.contains_key`).
- **Élevée — précédence OR/AND dans filtres multiples** : `is_empty`/`is_not_empty` émettaient
  `col IS NULL OR col = 0` sans parenthèses → combiné à `AND`, un filtre `is_empty(B) AND
  is_not_empty(A)` matchait à tort. Fix : parenthéser les clauses OR/AND.
- **Moyenne — select round-trip cassé** : `push_value` stockait l'id brut (`"opt_1"`),
  `read_cell` faisait `from_str` qui échouait → valeur relue `Null`. Fix : `push_value` stocke
  en JSON (`serde_json::to_string`), `read_cell` fallback sur la chaîne brute si parse échoue.
- **Moyenne — SWITCH sans défaut** : renvoyait la dernière valeur de cas au lieu de BLANK.
  Fix : la valeur de défaut n'est prise que si nombre d'arguments impair (target+paires+défaut).
- **Moyenne — champ link/lookup via modal "Nouvelle table"** : `create_table` traitait un
  champ link comme colonne TEXT stockée (pas d'`_relations`, pas de backlink). Fix :
  `create_table` rejette `link/lookup/rollup/count/formula` (erreur explicite) ;
  `FieldTypeSelector` prend un prop `storedOnly` (utilisé par `CreateTableModal`).

### Race schéma SQLite (tests instables)
- Symptôme : `cargo test` en parallèle échouait aléatoirement (`no such column` /
  `ColumnNotFound`) sur `full_flow`/`relations_flow`/`delete_records_cleanup_links`,
  alors que `--test-threads=1` passait. La colonne EXISTAIT (vérifié via
  `pragma_table_info` sur la même connexion).
- Cause : pool sqlx à 5 connexions ; un DDL (ALTER/CREATE/DROP) sur une connexion n'était pas
  vu par une autre connexion dont le schéma était figé (WAL + cache de schéma par connexion).
- Fix : `max_connections(1)` dans `connection.rs` (une seule connexion ⇒ schéma toujours
  cohérent). Correctifs annexes : `delete_table` lisait les champs lien via `pool` hors
  transaction → `&mut *tx` ; `populate_index` ne rouvre plus de 2e pool sur le même fichier
  (utilise le pool principal pour la base active) ; `Workspace::create` ne crée plus de pool
  temporaire jetable.

### Sprint 3 — UX / A11y
- **A1/A2** : `Modal.tsx` — `role="dialog"` + `aria-modal` + `aria-labelledby` + focus trap (Tab/Shift+Tab cyclique) + Échap ferme + restauration du focus précédent ✅
- **A1/A2** : `LinkPickerModal.tsx` — même traitement a11y (dialog + focus trap + Échap) ✅
- **A3** : `WorkspaceSidebar.tsx` — items `role="button"` + `tabIndex=0` + `onKeyDown` (Enter/Espace) + `aria-current` + `aria-label` + `role="list"` ✅
- **R5** : `tableStore.setActiveView` charge désormais la config de la vue (filtres/sorts/groups) — `FilterToolbar` expose un sélecteur de vues ✅
- **C8** : `DataGrid` affiche `error` (React Query) en `role="alert"` au lieu de `console.error` silencieux ; `aria-live` sur chargement/erreur ✅
- **A10** : `CreateTableModal`/`CreateFieldModal`/`CreateViewModal`/`CreateDatabaseModal` — `busy` state, boutons désactivés pendant l'envoi, labels `Création…`/`Ajout…` (anti double-clic) ✅
- **E2** : `Cell.tsx` — nombre invalide détecté à la validation, message `Nombre invalide` + `aria-invalid` + bordure danger, commit bloqué ✅
- **A4** : `styles.css` — `focus-visible` avec `outline 2px solid var(--accent)` + `outline-offset` sur input/select/textarea/button/sidebar-item ; ajout `prefers-reduced-motion` ✅
- **A9** : `lib/formatError.ts` — dé-préfixe `Error:` et nettoie les messages ; remplace `String(e)` brut dans tous les modals ✅
- **E7** : `FilterToolbar` chips clés stables (`field_id:operator:value:i`) ✅
- **A5** : boutons × avec `aria-label` ✅
- Vérifs : `cargo test` 16/16 (15 runs), `pnpm build` OK, clippy 10 warnings pré-existants

### Sprint 4 — Performance
- **P1/P4** : `repository.rs:get_table_data` — cache `HashMap<String, Vec<Field>>` (`cached_fields`) passé à `resolve_link_displays`/`compute_computed_fields`/`fetch_primary_displays`/`fetch_target_values` pour éviter N+1 `list_fields` ✅ — `src-tauri/src/db/repository.rs:586`
- **P2** : `compute_backlink_field` `src-tauri/src/db/repository.rs:1255` — remplace `SELECT _id, col FROM source_table` (full scan) par `WHERE instr(col, '"record_id":"<id>"')>0` filtré sur les `target_ids` visibles (page), + filtre `target_ids.contains` en Rust ✅
- **P5** : `upsert_records` `src-tauri/src/db/repository.rs:597` — batch multi-rows si uniformité des champs présents (`sigs` égales + `len>1`) : un seul `INSERT ... VALUES (...),(... ) ON CONFLICT DO UPDATE`, sinon fallback loop par record ; garde sémantique `DO UPDATE SET` présent-only ✅
- **C1-C3** : `DataGrid.tsx:30` — `sortDir`/`onToggleSort`/`onCommit`/`onDeleteRow`/`onAddField` en `useCallback`, `meta` en `useMemo` stable (évite re-renders colonnes/cellules) ✅
- **P6** : `hooks/useDebouncedValue.ts` (300ms) utilisé par `LinkPickerModal.tsx:36` pour la recherche ; `FilterToolbar` ajout explicite évite spam backend, debounce disponible pour futurs filtres live ✅
- **P5 front** : `invalidateQueries` ciblé `src/components/fields/CreateTableModal.tsx:50` `["table-data", dbId]`+`["tables", dbId]`, `CreateFieldModal.tsx:139` `["table-data", dbId, tableId]`+`["fields",...]`, `LinkPickerModal.tsx:158` `["table-data", dbId, field.table_id]` (fini global) ✅ — `src/hooks/useTableData.ts:25`
- **E5** : `LinkPickerModal.tsx:91` — recherche texte + pagination (page 50, total, Précédent/Suivant, `filters: [{field_id: primary, operator:"contains"}]`, `page` param, `total` affiché, reset page sur `debouncedQuery`) — plus de plafond 1000 muet ✅
- **R1** : `hooks/useTable.ts` unifié sur React Query (`useQuery ["tables"]`/`["fields"]`/`["views"]` + `useEffect` sync store) + doc R1 ; `commands/mod.rs:6` extrait `active_pool` partagé (supprime duplication `tables.rs`/`views.rs`) ✅ — `src/hooks/useTable.ts:1`
- Vérifs : `cargo test` 16/16, `cargo clippy` 10 warnings pré-existants, `pnpm build` OK (115 modules)

### Phase 3 — Cross-DB / référentiels (MVP)
- `models.rs:175` `LinkFieldConfig.target_db_id` (default "") + `models.rs:162` `LinkValue.db_id` déjà, `src-tauri/migrations/0001` `source_db_id`/`target_db_id` existants — plumbing `repository.rs:337` `create_link_field` stocke `target_db_id` dans config + `_relations.target_db_id`, expose `target_db_id` dans `forward_config`, `is_backlink` skip si cross-DB ✅ — `src-tauri/src/db/repository.rs:337`
- `repository.rs:337` `create_table` `src-tauri/src/db/repository.rs:45` étendu `source_db_id: Option<String>` (stocké dans `_tables.source_db_id`, retour `Table.source_db_id`) — `commands/tables.rs:20` param `source_db_id: Option<String>` + `src/lib/api.ts:40` `sourceDbId?` ✅
- `link_records` `src-tauri/src/db/repository.rs:444` écrit `LinkValue.db_id = Some(target_db_id)` si cross-DB, sinon `None` ✅
- `get_record_with_relations` `src-tauri/src/db/repository.rs:1537` `depth 0-3` — fetch base `SELECT _id + stored` + `resolve_link_displays` + `collect_relations` via `fetch_full_records` `src-tauri/src/db/repository.rs:1654` pour chaque lien forward, depth>1 préparé (sans récursion mutuelle Box::pin) ✅ — `src-tauri/src/db/models.rs:259` `RecordWithRelations` + `src-tauri/src/commands/tables.rs:32` `get_record_with_relations` + `src-tauri/src/lib.rs:44` handler + `src/lib/api.ts:49` ✅
- Front cross-DB : `src/types/field.ts:106` `target_db_id?` + `src/components/fields/CreateFieldModal.tsx:43` sélecteur Base cible (si `databases.length>1`, `targetDbId` + `availableTables` fetch `listTables(targetDbId)`) + `createLinkField` passe `target_db_id` ✅
- Test `tests.rs:655` `get_record_with_relations_depth1` (intra 2 tables, link many, 2 cibles, vérif `relations[link].len=2` + `source_db_id` plumbing) ✅
- Vérifs : `cargo test` 17/17, `pnpm build` OK (115 modules)

### Audit — 8 remarques corrigées (revue code)
- **CRITIQUE — reads cross-DB pointaient sur le pool de la base active** : `resolve_link_displays`/`compute_lookup_field`/`compute_rollup_field`/`collect_target_values`/`collect_relations` requêtaient le pool courant même quand `target_db_id != ""` → `no such table` et la grille entière plantait via `?`. Fix : paramètres `db_pools: &HashMap<String, SqlitePool>` + `current_db_id` threadés dans `get_table_data`/`get_record_with_relations`, helper `pool_for()` (`''`→pool courant, sinon pool cible) ; base cible indisponible → skip propre (displays/valeurs vides, jamais d'erreur grille). Côté commands : `pool_for_db` (ouvre le pool d'une autre base, fini la restriction "base active") + `other_db_pools` (`commands/mod.rs:20,34`). `src/lib/api.ts` + `src/types/field.ts` déjà alignés. ✅ — `src-tauri/src/db/repository.rs:1126,1172,1227,1310,1659`
- **HAUTE — `create_field` acceptait `link` sans relation** (`_relations`/backlink) → colonne TEXT non nettoyable. Fix : `create_field` rejette `link` (create_link_field requis) ; `lookup/rollup/count/formula` calculés restent autorisés. ✅ — `src-tauri/src/db/repository.rs:208`
- **LinkPickerModal lisait la cible via la base active** même cross-DB. Fix : `fetchDbId = cfg.target_db_id || dbId` pour `listFields`/`getTableData` ; `linkRecords`/`unlinkRecords` restent sur la base active (champ source). ✅ — `src/components/grid/LinkPickerModal.tsx:36`
- **Cell/LinkCell non opérables clavier** (a11y, Sprint 3). Fix : `role="button"` + `tabIndex` + `onKeyDown` (Enter/Espace) sur Cell (dont checkbox `aria-checked`) et LinkCell + `.cell:focus-visible` dans styles.css. ✅ — `src/components/grid/Cell.tsx:71,99` / `src/components/grid/LinkCell.tsx:21`
- **Select créé dans CreateTableModal sans config** → dropdown vide sans issue. Fix : `config: { options: [] }` pour les champs `select` à la création. ✅ — `src/components/fields/CreateTableModal.tsx:40`
- **WorkspaceLauncher affichait `String(e)` brut**. Fix : `formatError(e)` (comme les modals). ✅ — `src/components/workspace/WorkspaceLauncher.tsx:17,29`
- **`src-tauri/gen/schemas/*.json` (artefacts build) commitables**. Fix : `src-tauri/gen` ajouté à `.gitignore`. ✅
- Noté sans fix : `visible_field_ids` stocké mais jamais appliqué (feature champs visibles), `csp: null` dans tauri.conf.json (pas de contenu distant aujourd'hui).

### Phase 4 — IA hybride (MVP)
- `ai/provider.rs` — `LLMProvider` trait + `LMStudioProvider` (OpenAI-compatible `base_url/model`, découverte `/v1/models`, `size_limit`) + `OpenAIProvider` (`api_key/model`) + `get_provider()` hybride (LM Studio ping → fallback OpenAI → heuristique offline) + `check_status()` + `ProviderStatus` — `src-tauri/src/ai/provider.rs:1`
- `ai/context_builder.rs` — `AICrossDBContext` (active + refs, `relations`, `sample` 50 lignes/table via `fetch_sample`), helper `context_prompt()` pour few-shot LLM — `src-tauri/src/ai/context_builder.rs:1`
- `ai/relation_suggest.rs` — heuristique noms normalisés + `value_overlap` + bonus `is_id_like` + cross-DB, top 10 filtrés (threshold 0.45 cross / 0.5 intra), raison + confidence — tests 2/2 — `src-tauri/src/ai/relation_suggest.rs:1`
- `ai/formula.rs` — `generate_heuristic()` mapping NL→formule (IF/SUM/AVERAGE/CONCATENATE/UPPER/LOWER/LEN/DATETIME_DIFF/ROUND) + validation `formula::parse`, `generate_formula()` tente LLM (JSON `expression/explanation`) sinon heuristique — `src-tauri/src/ai/formula.rs:1`
- `ai/analysis.rs` — stats locales (`row_count`, `non_null/nulls/distinct/top_values`, `NumericStats min/max/avg/sum`), insights `heu_insights` (nulls >30%, min négatif, table vide), `question` booste filtres, LLM enrichi si dispo — `src-tauri/src/ai/analysis.rs:1`
- `ai/cleaning.rs` — `heuristic_plan()` (trim/upper/lower/normalize_email/normalize_phone/deduplicate/fill_null via mots-clés FR/EN), `build_preview()` (5 lignes, 10 max), `preview_with_llm()` + `apply_transform()` transactionnel (instr/col, dedup via `DELETE` groupé, else `upsert_records` par record) — `src-tauri/src/ai/cleaning.rs:1`
- `commands/ai.rs` — 6 commandes Tauri : `ai_suggest_relations`, `ai_generate_formula`, `ai_analyze`, `ai_clean_preview`, `ai_apply_transform`, `ai_check_status` — `src-tauri/src/commands/ai.rs:1`
- Dépendances : `async-trait 0.1 + reqwest 0.12 (json, rustls-tls)` — `src-tauri/Cargo.toml:28`
- Front : `src/types/ai.ts` + `src/lib/api.ts:162` (+6 invokes) + `src/hooks/useAI.ts` + `src/components/ai/*` (AIAssistant drawer 420px + 4 tabs + status, FormulaGenerator, AnalysisPanel, CleaningWizard preview→apply, RelationSuggest) + `src/stores/uiStore.ts:10` `aiOpen` + `src/components/MainLayout.tsx:27` bouton ✦ IA — `src/components/ai/AIAssistant.tsx:1`
- Tests : `tests.rs:655` `ai_phase4_flow` (suggest + formula IF/SUM + analysis + cleaning trim/normalize vérifiés) ✅
- Vérifs : `cargo test` 25/25 (dont 7 IA), `pnpm build` 122 modules

### Audit Phase 4 — 6 remarques corrigées (revue IA)
- **H1 — RwLock workspace tenu pendant les appels HTTP LLM** : `commands/ai.rs` gardait `workspace.read()` à travers `get_provider` (ping 2s) + `complete` (timeout 60s) → `switch_database` bloqué. Fix : helper async `snapshot_workspace` qui clone `(config, dir)` puis drop le garde avant tout réseau. ✅ — `src-tauri/src/commands/ai.rs:19`
- **H2 — RegexReplace compilait une regex sans limite depuis le front** (contredit politique S1 d'evaluator). Fix : `compile_regex_bounded` (len ≤ 4096, size_limit + dfa_size_limit 1 Mo). ✅ — `src-tauri/src/ai/cleaning.rs:247`
- **H3 — Backlinks comptés dans les stats** → faux insights « >30% vides » à chaque analyse. Fix : `compute_stats` skippe désormais lookup/rollup/count/formula **et** link backlink (pas de colonne SQL → absents du sample). ✅ — `src-tauri/src/ai/analysis.rs:45`
- **H4 — Ops LLM de nettoyage non filtrées** : champ inexistant passait le preview puis échouait en SQL à l'application ; champ calculé accepté aussi. Fix : `all_fields` ne renvoie que les champs stockés (`is_stored`) et `preview_with_llm` rejette toute op sur champ inconnu (fallback heuristique si plan vide). ✅ — `src-tauri/src/ai/cleaning.rs:69,389`
- **M1 — `useAISuggest` mort et cassé** (`window.__sebtable_dbId` jamais écrit). Supprimé ; RelationSuggest utilisait déjà sa propre query. ✅ — `src/hooks/useAI.ts`
- **M2 — Cardinalité factice + condition morte** dans relation_suggest (`|| true`, deux branches `"many"`, `tgt_primary` inutilisé). Fix : `existing_relation(s_field, t_db, t_table)` propre ; cardinalité réelle = ratio d'unicité des valeurs source (>0.9 → "one", sinon "many"). ✅ — `src-tauri/src/ai/relation_suggest.rs:95,192`
- **M6 — Zéro nouveau warning clippy** : imports morts (`Field`/`Table`/`HashMap`), `stored_ids`, `mut msgs` + reliquat, méthodes mortes `table_by_id`/`max_context_tokens` supprimés ; + 4 warnings clippy des nouveaux fichiers corrigés (sort_by_key, ifs collabés, matches!). Reste uniquement les 10 pré-existants de `formula/evaluator.rs`. ✅
- Vérifs post-fix : `cargo test` **25/25**, clippy 10 (pré-existants seulement), `pnpm build` OK (122 modules)

### Audit Phase 4 — round 2 (M3/M4/M5)
- **M3 — Formules marquées valides avec champs inexistants** : `missing_fields()` extrait les `{Réfs}` et vérifie l'existence dans la table ; `valid = parse OK ∧ champs tous présents` (chemin heuristique ET LLM). Fallback par défaut construit sur les champs réels (select/text + number) au lieu du `{Statut}` inventé ; si aucun champ compatible → résultat explicite `valid:false`. Les prompts déjà formulés (`SUM({...})`…) sont validés tels quels AVANT les heuristiques (sinon réécriture en silence). Branches heuristiques sans champ correspondant → `None` au lieu d'un nom de champ inventé. ✅ — `src-tauri/src/ai/formula.rs`
- **M4 — Dédup muette sur colonnes numériques + N+1 upserts** : lecture tolérante aux types (String/f64/i64/null) au lieu de `query_as<(String,String)>` qui no-op'ait silencieusement sur REAL/INTEGER ; upserts groupés en **un seul** appel `upsert_records`. Test d'intégration : doublon numérique détecté/supprimé, valeurs finales vérifiées. ✅ — `src-tauri/src/ai/cleaning.rs:427,520`
- **M5 — `affected_rows` mensonger (= nb d'aperçus)** : renommé `estimated_rows`, estimé sur TOUT le sample (lignes modifiées par ≥1 op + doublons dédup), extrapolé si sample partiel (`row_count > sample`). UI : « Appliquer (≈ N ligne(s) estimée(s)) ». ✅ — `src-tauri/src/ai/cleaning.rs:344` / `src/types/ai.ts` / `CleaningWizard.tsx`
- Vérifs post-fix : `cargo test` **25/25**, clippy 10 (pré-existants seulement), `pnpm build` OK

### Mineurs restants — 5 points corrigés
- **`dir` inutilisé dans `build_context`** : param `dir: &Path` jamais lu (`let _ = dir`) ; supprimé, les 4 call sites `commands/ai.rs` passent désormais `snapshot_config` (plus besoin de `dir`). Test `tests.rs` mis à jour. ✅ — `src-tauri/src/ai/context_builder.rs:56` / `src-tauri/src/commands/ai.rs:17`
- **Pools cross-DB recréés à chaque appel AI** : `other_db_pools` rouvrait un `open_pool` par base à chaque `build_context`. Fix : cache `AppState::cross_pools: RwLock<HashMap<String, SqlitePool>>` (clone `Arc` interne), invalidé sur `create/open/switch_workspace`. Snapshot des `(databases, dir)` hors lock pour ne pas bloquer. ✅ — `src-tauri/src/lib.rs:18` / `src-tauri/src/commands/mod.rs:37` / `workspace.rs:19,45`
  - Complément (audit round 3) : **M-A** — `switch_database` n'invalide pas le cache → 2 pools vivants sur le fichier devenu actif (viole l'invariant WAL de `manager.rs:148`, le cache alimente aussi `get_table_data`) ; fix = `clear()` dans la commande. **M-D** — `active_name` sensible à la casse alors que `get_provider` normalise (`"Hybrid"` → statut « heuristic » à tort) ; fix = `to_lowercase()` dans le helper. ✅ — `workspace.rs:45` / `provider.rs:170`
- **Client `reqwest` par requête** : 3 `Client::builder().build()` par appel (LMStudio 60s + discovery 3s + availability 2s). Fix : `shared_client(timeout)` via `OnceLock` (un client par timeout réutilisé). ✅ — `src-tauri/src/ai/provider.rs:38`
- **`check_status` dupliquait `get_provider`** : logique `is_openai_configured` / `active_name` copiée + branche morte `if mode == "lmstudio"` dans le bloc `openai||hybrid`. Fix : helpers `is_openai_configured` + `active_name`, `get_provider` restructuré en branches `hybrid`/`openai`/`lmstudio` disjointes. ✅ — `src-tauri/src/ai/provider.rs:161`
- **`RelationSuggest` affichait l'id brut** : `hint` ne montrait que `target_table_id` (`tbl_xxx`). Fix : `target_table_name (id court…)` — l'utilisateur voit le nom, l'id reste repérable. ✅ — `src/components/ai/RelationSuggest.tsx:27`
- Vérifs post-fix : `cargo test` **25/25**, clippy 10 (pré-existants seulement), `pnpm build` OK (122 modules)

### Phase 5 — Polish & Avancé (MVP)
- **Import/Export** : `commands/import_export.rs` — `export_table(db_id, table_id, format)` (csv/json/xlsx via `csv`/`serde_json`/`rust_xlsxwriter`, tous les champs stockés, tri `_id`) + `import_table(db_id, file, options)` (csv via `csv::Reader`, json via `serde_json`, xlsx via `calamine`, mapping nom→field_id insensible à la casse, création de table si `table_id=None`, limite 10k lignes / 20 Mo) — `src-tauri/Cargo.toml: csv 1.3 + calamine 0.26 + rust_xlsxwriter 0.78` ✅
- **Attachments** : `commands/attachments.rs` — champ `attachment` (`{max_size_mb}` en config, `is_stored()=true`, JSON `[{name,url,size,type}]` en colonne TEXT), `upload_attachment` (sanitize, dossier `attachments/<db_id>/<table_id>/<record_id>/`, suffixe si collision, `mime_guess`, MAJ JSON de la cellule, 10 Mo max), `list_attachments`/`delete_attachment` (MAJ JSON + `remove_file` best-effort) — `mime_guess 2` ✅ — front `AttachmentCell.tsx` (chips + upload/delete, re-render via refetch) + `ColumnDefFactory` branche `attachment`
- **Settings & raccourcis** : `workspace.rs:71` `get_workspace_settings` + `update_workspace_settings` (validation `llm_provider` + `ws.save()` rendu `pub(crate)`) ; front `SettingsModal.tsx` (form 5 champs + validation, hallucination minime) + `useShortcuts.ts` (`Ctrl+K` focus recherche, `Ctrl+N` table, `Ctrl+,` settings, `Ctrl+Shift+I` IA) branché dans `MainLayout`, barre haute boutons `⇅ Import` + `⚙ Paramètres` — `src/lib/api.ts:212` (+7 invokes) ✅
- **Packaging** : `tauri.conf.json` enrichi (`publisher`, `category: Office`, `shortDescription/longDescription`, `wix.language fr-FR`) + icons déjà présents ✅
- **Tests** : `tests.rs:852` `phase5_import_export_attachments` (create table attachment, CSV génération/parsing + import 2→4 lignes, XLSX ZIP header `PK`, JSON round-trip, fichier disque + JSON cellule + settings persistence) ✅
- Vérifs : `cargo test` **26/26**, `pnpm build` OK (126 modules)

### Vérifications actives
- `cargo test` : **26/26 OK** (8 lib + 18 intégration, dont Phase 5), stable
- `pnpm build` : OK (126 modules)
- Clippy : 10 warnings pré-existants (`formula/evaluator.rs` uniquement) + 2 `allow(dead_code)` sur helpers import

---

## ❌ À FAIRE

### Sprint 3 — UX / A11y — ✅ fait (voir section FAIT)
### Sprint 4 — Performance — ✅ fait (voir section FAIT)

### Phase 3 — Cross-DB / référentiels — ✅ fait (MVP, voir FAIT)

### Phase 4 — IA hybride — ✅ fait (MVP, voir FAIT)
### Phase 5 — Polish & Avancé — ✅ fait (MVP, voir FAIT)

### Améliorations faibles (optional)
- [x] E1 : `new_id` → UUID complet (32 hex) `src-tauri/src/utils/mod.rs:5` ✅
- [x] E3 : division par zéro → `Op::Div`/`Op::Mod` renvoient `Value::Null` `src-tauri/src/formula/evaluator.rs:73,81` ✅
- [x] E5 : `LEN`/`LEFT`/`RIGHT` en grapheme clusters (`unicode-segmentation`) `src-tauri/src/formula/evaluator.rs:234,238,243` ✅
- [x] E8 : `round_to` garde-fou overflow/non-fini `src-tauri/src/formula/evaluator.rs:409` ✅
- [x] A4/A8 : `prefers-reduced-motion` dans styles.css ✅ (Sprint 3)
- [x] R3 : ErrorBoundary React au top niveau `src/components/ErrorBoundary.tsx` + `src/main.tsx:14` ✅
- [x] csp `null` → CSP Tauri raisonnable `src-tauri/tauri.conf.json:22` ✅
- [x] R6 : pagination hors `ViewConfig` ✅ (Sprint 3 `setActiveView` remet `page: null`)
- [x] A5 : `aria-label` sur boutons × (CreateTableModal) ✅
- [x] A6 : `aria-live` sur états chargement/erreur ✅
- [x] E7 : `key={i}` sur filtres → clé stable ✅
- [ ] A4 : messages d'erreur localisés FR (backend FR / audit noté)
- [ ] visible_field_ids stocké mais jamais appliqué (feature champs visibles — reportée, voir Phase suivante)

### Dead code / dette
- [ ] **A2** : `include_lookups: bool` dans `get_table_data` commande — implémenter ou supprimer
- [x] **A1** : `active_pool` dupliqué (`commands/tables.rs` + `commands/views.rs`) → extraire dans `commands/mod.rs` ✅ (Sprint 4)
- [ ] **A3** : `RwLock<Option<Workspace>>` contention → pool séparé (ArcSwap/RwLock<SqlitePool>)
- [ ] **A4** : `update_view` ne met à jour que la config, pas name/type

---

## Commandes de vérification
```
cargo test           # dans src-tauri/
cargo clippy         # warnings style pré-existants seulement
pnpm build           # tsc + vite
```
