// Seed d'une base de démo pour capture d'écran.
// Usage : cargo run --example seed_demo [chemin/du/dossier]  (dans src-tauri/)
// Si le dossier existe, il est écrasé.
use sebtable_lib::db::models::{FieldInput, LinkFieldConfig, LinkTarget, ViewConfig, ViewInput};
use sebtable_lib::db::repository;
use sebtable_lib::workspace::manager::Workspace;
use serde_json::{json, Value};
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::path::Path;

fn obj(entries: Vec<(&str, Value)>) -> Value {
    Value::Object(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

fn select_opts(opts: &[(&str, &str, &str)]) -> Value {
    json!({
        "options": opts
            .iter()
            .map(|(id, name, color)| json!({"id": id, "name": name, "color": color}))
            .collect::<Vec<_>>()
    })
}

async fn field_map(pool: &SqlitePool, table_id: &str) -> HashMap<String, String> {
    repository::list_fields(pool, table_id)
        .await
        .unwrap()
        .into_iter()
        .map(|f| (f.name, f.id))
        .collect()
}

async fn seed_clients(pool: &SqlitePool, table_id: &str, f: &HashMap<String, String>) {
    let rows: Vec<(String, &str, &str, &str, &str, &str, f64, bool, &str)> = vec![
        ("rec_cl_01".into(), "Sophie Martin", "sophie.martin@example.fr", "Lyon", "opt_fidele", "2019-03-12", 48200.0, true, "Compte entreprise"),
        ("rec_cl_02".into(), "Julien Bernard", "julien.bernard@example.fr", "Paris", "opt_actif", "2021-07-01", 21500.0, true, "Contact marketing"),
        ("rec_cl_03".into(), "Claire Dubois", "claire.dubois@example.fr", "Bordeaux", "opt_prospect", "2024-11-05", 0.0, false, "Démo planifiée"),
        ("rec_cl_04".into(), "Thomas Petit", "thomas.petit@example.fr", "Toulouse", "opt_actif", "2020-01-20", 33900.0, true, "Renouvellement en cours"),
        ("rec_cl_05".into(), "Léa Moreau", "lea.moreau@example.fr", "Nantes", "opt_fidele", "2018-06-15", 61400.0, true, "Client référence"),
        ("rec_cl_06".into(), "Antoine Leroy", "antoine.leroy@example.fr", "Lille", "opt_actif", "2022-09-03", 18750.0, true, ""),
        ("rec_cl_07".into(), "Emma Roux", "emma.roux@example.fr", "Marseille", "opt_prospect", "2025-01-10", 0.0, false, "Relancer fin de mois"),
        ("rec_cl_08".into(), "Hugo Fournier", "hugo.fournier@example.fr", "Strasbourg", "opt_perdu", "2017-11-22", 9200.0, false, "Churn : budget"),
        ("rec_cl_09".into(), "Manon Girard", "manon.girard@example.fr", "Rennes", "opt_actif", "2023-04-18", 12600.0, true, ""),
        ("rec_cl_10".into(), "Lucas Bonnet", "lucas.bonnet@example.fr", "Nice", "opt_fidele", "2019-10-08", 40800.0, true, "Utilise l'API"),
        ("rec_cl_11".into(), "Chloé Lambert", "chloe.lambert@example.fr", "Montpellier", "opt_prospect", "2025-02-14", 0.0, false, ""),
        ("rec_cl_12".into(), "Nathan Morel", "nathan.morel@example.fr", "Dijon", "opt_actif", "2022-12-01", 27400.0, true, "2e licence à ajouter"),
    ];
    let mut records = Vec::new();
    for (id, nom, email, ville, statut, depuis, ca, actif, notes) in rows {
        records.push(obj(vec![
            ("_id", json!(id)),
            (f["Nom"].as_str(), json!(nom)),
            (f["Email"].as_str(), json!(email)),
            (f["Ville"].as_str(), json!(ville)),
            (f["Statut"].as_str(), json!(statut)),
            (f["Depuis"].as_str(), json!(depuis)),
            (f["CA annuel"].as_str(), json!(ca)),
            (f["Actif"].as_str(), json!(actif)),
            (f["Notes"].as_str(), json!(notes)),
        ]));
    }
    repository::upsert_records(pool, table_id, records).await.unwrap();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| r"C:\Users\sebas\Downloads\sebtable-demo".into());
    let dir = Path::new(&dir);
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    let mut ws = Workspace::create(dir, "Démo Sebtable".into()).await?;
    ws.config.databases[0].name = "CRM Démo".into();
    let config_json = serde_json::to_string_pretty(&ws.config.masked())?;
    std::fs::write(dir.join("workspace.json"), config_json)?;
    let pool = ws.pool.clone();

    // ---- Table Clients ----
    let clients = repository::create_table(
        &pool,
        "Clients".into(),
        vec![
            FieldInput { name: "Nom".into(), field_type: "text".into(), config: json!({}) },
            FieldInput { name: "Email".into(), field_type: "email".into(), config: json!({}) },
            FieldInput { name: "Ville".into(), field_type: "text".into(), config: json!({}) },
            FieldInput { name: "Statut".into(), field_type: "select".into(), config: select_opts(&[("opt_actif", "Actif", "#2e9e5b"), ("opt_prospect", "Prospect", "#f5a623"), ("opt_fidele", "Fidèle", "#4f8cff"), ("opt_perdu", "Perdu", "#e15c5c")]) },
            FieldInput { name: "Depuis".into(), field_type: "date".into(), config: json!({}) },
            FieldInput { name: "CA annuel".into(), field_type: "number".into(), config: json!({}) },
            FieldInput { name: "Actif".into(), field_type: "checkbox".into(), config: json!({}) },
            FieldInput { name: "Notes".into(), field_type: "long_text".into(), config: json!({}) },
        ],
        None,
    )
    .await?;
    let cf = field_map(&pool, &clients.id).await;
    seed_clients(&pool, &clients.id, &cf).await;

    // ---- Table Commandes ----
    let commandes = repository::create_table(
        &pool,
        "Commandes".into(),
        vec![
            FieldInput { name: "Réf".into(), field_type: "text".into(), config: json!({}) },
            FieldInput { name: "Montant".into(), field_type: "number".into(), config: json!({}) },
            FieldInput { name: "Date".into(), field_type: "date".into(), config: json!({}) },
            FieldInput { name: "Statut".into(), field_type: "select".into(), config: select_opts(&[("opt_payee", "Payée", "#2e9e5b"), ("opt_attente", "En attente", "#f5a623"), ("opt_annulee", "Annulée", "#e15c5c")]) },
        ],
        None,
    )
    .await?;
    let mf = field_map(&pool, &commandes.id).await;

    let link = repository::create_link_field(
        &pool,
        &commandes.id,
        "Client".into(),
        LinkFieldConfig {
            target_table_id: clients.id.clone(),
            target_db_id: String::new(),
            cardinality: "one".into(),
            allow_creating: true,
            is_backlink: false,
            source_link_field_id: String::new(),
        },
    )
    .await?;

    repository::create_field(
        &pool,
        &commandes.id,
        FieldInput { name: "TVA".into(), field_type: "formula".into(), config: json!({"expression": "ROUND({Montant} * 0.2, 2)"}) },
    )
    .await?;
    repository::create_field(
        &pool,
        &commandes.id,
        FieldInput { name: "Total TTC".into(), field_type: "formula".into(), config: json!({"expression": "ROUND({Montant} * 1.2, 2)"}) },
    )
    .await?;

    let cmds: Vec<(String, &str, f64, &str, &str, &str)> = vec![
        ("rec_cd_01".into(), "CMD-2024-001", 1200.00, "2024-02-10", "opt_payee", "rec_cl_01"),
        ("rec_cd_02".into(), "CMD-2024-002", 450.50, "2024-03-01", "opt_payee", "rec_cl_02"),
        ("rec_cd_03".into(), "CMD-2024-003", 890.00, "2024-03-15", "opt_payee", "rec_cl_04"),
        ("rec_cd_04".into(), "CMD-2024-004", 2340.00, "2024-04-02", "opt_payee", "rec_cl_05"),
        ("rec_cd_05".into(), "CMD-2024-005", 175.20, "2024-04-20", "opt_attente", "rec_cl_06"),
        ("rec_cd_06".into(), "CMD-2024-006", 690.00, "2024-05-11", "opt_payee", "rec_cl_01"),
        ("rec_cd_07".into(), "CMD-2024-007", 310.00, "2024-05-28", "opt_annulee", "rec_cl_08"),
        ("rec_cd_08".into(), "CMD-2024-008", 980.00, "2024-06-09", "opt_payee", "rec_cl_09"),
        ("rec_cd_09".into(), "CMD-2024-009", 1520.75, "2024-06-25", "opt_attente", "rec_cl_10"),
        ("rec_cd_10".into(), "CMD-2024-010", 430.00, "2024-07-07", "opt_payee", "rec_cl_12"),
        ("rec_cd_11".into(), "CMD-2024-011", 2750.00, "2024-07-19", "opt_payee", "rec_cl_05"),
        ("rec_cd_12".into(), "CMD-2024-012", 1180.00, "2024-08-02", "opt_attente", "rec_cl_01"),
        ("rec_cd_13".into(), "CMD-2024-013", 650.00, "2024-08-16", "opt_payee", "rec_cl_04"),
        ("rec_cd_14".into(), "CMD-2024-014", 820.00, "2024-09-01", "opt_attente", "rec_cl_07"),
    ];
    let mut records = Vec::new();
    for (id, rf, montant, date, statut, _) in &cmds {
        records.push(obj(vec![
            ("_id", json!(id)),
            (mf["Réf"].as_str(), json!(rf)),
            (mf["Montant"].as_str(), json!(montant)),
            (mf["Date"].as_str(), json!(date)),
            (mf["Statut"].as_str(), json!(statut)),
        ]));
    }
    repository::upsert_records(&pool, &commandes.id, records).await.unwrap();
    for (id, _, _, _, _, client) in &cmds {
        repository::link_records(
            &pool,
            &link.id,
            id,
            vec![LinkTarget { record_id: client.to_string() }],
        )
        .await
        .unwrap();
    }

    // ---- Table Produits ----
    let produits = repository::create_table(
        &pool,
        "Produits".into(),
        vec![
            FieldInput { name: "Nom".into(), field_type: "text".into(), config: json!({}) },
            FieldInput { name: "Catégorie".into(), field_type: "select".into(), config: select_opts(&[("opt_logiciel", "Logiciel", "#4f8cff"), ("opt_service", "Service", "#9b6df2"), ("opt_formation", "Formation", "#f5a623"), ("opt_conseil", "Conseil", "#2e9e5b")]) },
            FieldInput { name: "Prix".into(), field_type: "number".into(), config: json!({}) },
            FieldInput { name: "Stock".into(), field_type: "number".into(), config: json!({}) },
            FieldInput { name: "Disponible".into(), field_type: "checkbox".into(), config: json!({}) },
        ],
        None,
    )
    .await?;
    let pf = field_map(&pool, &produits.id).await;
    let prods: Vec<(&str, &str, f64, i64, bool)> = vec![
        ("Formule de gestion Pro", "opt_logiciel", 149.00, 120, true),
        ("Pack reporting annuel", "opt_service", 390.00, 500, true),
        ("Formation Excel avancée", "opt_formation", 250.00, 30, true),
        ("Licence utilisateur", "opt_logiciel", 45.00, 1000, true),
        ("Support prioritaire", "opt_service", 99.00, 200, true),
        ("Audit données", "opt_conseil", 890.00, 15, true),
        ("Kit onboarding", "opt_formation", 75.00, 80, true),
        ("Extension API", "opt_logiciel", 199.00, 60, false),
    ];
    let mut records = Vec::new();
    for (i, (nom, cat, prix, stock, dispo)) in prods.iter().enumerate() {
        records.push(obj(vec![
            ("_id", json!(format!("rec_pr_{:02}", i + 1))),
            (pf["Nom"].as_str(), json!(nom)),
            (pf["Catégorie"].as_str(), json!(cat)),
            (pf["Prix"].as_str(), json!(prix)),
            (pf["Stock"].as_str(), json!(stock)),
            (pf["Disponible"].as_str(), json!(dispo)),
        ]));
    }
    repository::upsert_records(&pool, &produits.id, records).await.unwrap();

    // ---- Vues grid par défaut ----
    for (tid, name) in [
        (&clients.id, "Grille Clients"),
        (&commandes.id, "Grille Commandes"),
        (&produits.id, "Grille Produits"),
    ] {
        repository::create_view(
            &pool,
            ViewInput { table_id: tid.clone(), name: name.into(), view_type: "grid".into(), config: ViewConfig::default() },
        )
        .await
        .unwrap();
    }

    ws.pool.close().await;
    println!("Seed OK : {}", dir.display());
    Ok(())
}
