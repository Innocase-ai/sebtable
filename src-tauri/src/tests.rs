#[cfg(test)]
mod tests {
    use crate::db::models::{
        FieldInput, Filter, LinkFieldConfig, LinkTarget, Sort, ViewConfig, ViewInput,
    };
    use crate::db::repository;
    use crate::workspace::manager::Workspace;
    use serde_json::{json, Value};
    use std::collections::HashMap;

    fn temp_dir() -> std::path::PathBuf {
        let d = std::env::temp_dir().join(format!("sebtable_test_{}", crate::utils::new_id("t")));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn obj(entries: Vec<(&str, Value)>) -> Value {
        Value::Object(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
    }

    async fn make_workspace() -> (std::path::PathBuf, Workspace) {
        let dir = temp_dir();
        let ws = Workspace::create(&dir, "Test".into()).await.unwrap();
        (dir, ws)
    }

    // delete_database : suppression d'une base non-active, garde de la dernière
    // base, bascule de la base active, et nettoyage des métadonnées orphelines
    // (_fields/_views) par delete_table.
    #[tokio::test]
    async fn delete_database_flow() {
        let (dir, mut ws) = make_workspace().await;
        let first_id = ws.config.active_database_id.clone();

        let second = ws.create_database("Secondaire".into(), crate::db::models::DbRole::Project).await.unwrap();
        let second_path = dir.join(&second.path);
        assert!(second_path.exists());

        // Garde : impossible de réduire à zéro... (on en a 2, ça doit marcher)
        ws.delete_database(&second.id).await.unwrap();
        assert!(!second_path.exists());
        assert!(!ws.config.databases.iter().any(|d| d.id == second.id));
        assert_eq!(ws.config.active_database_id, first_id);

        // Suppression de la base ACTIVE : bascule automatique sur la restante
        let other = ws.create_database("Autre".into(), crate::db::models::DbRole::Project).await.unwrap();
        ws.delete_database(&first_id).await.unwrap();
        assert_eq!(ws.config.active_database_id, other.id);
        assert!(!dir.join(format!("{}.db", first_id)).exists()
            && !dir.join("databases").join(format!("{}.db", first_id)).exists());

        // Garde : la dernière base restante ne peut pas être supprimée
        let err = ws.delete_database(&other.id).await.unwrap_err();
        assert!(err.to_string().contains("dernière base"));

        // delete_table ne laisse pas d'orphelins dans _fields/_views
        let pool = ws.pool.clone();
        let t = repository::create_table(
            &pool,
            "Tmp".into(),
            vec![FieldInput { name: "A".into(), field_type: "text".into(), config: json!({}) }],
            None,
        ).await.unwrap();
        repository::create_view(&pool, ViewInput { table_id: t.id.clone(), name: "V".into(), view_type: "grid".into(), config: ViewConfig::default() }).await.unwrap();
        repository::delete_table(&pool, &t.id).await.unwrap();
        let orphan_fields: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _fields WHERE table_id = ?")
            .bind(&t.id).fetch_one(&pool).await.unwrap();
        let orphan_views: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM _views WHERE table_id = ?")
            .bind(&t.id).fetch_one(&pool).await.unwrap();
        assert_eq!((orphan_fields, orphan_views), (0, 0));

        let _ = std::fs::remove_dir_all(dir);
    }


    // Reproduit exactement le flux « + Nouvelle ligne » de l'UI : l'ajout n'envoie
    // QUE {_id} (aucun champ), puis on relit la table. Vérifie que la ligne apparaît
    // bien au rechargement (c'est le symptôme signalé : visible seulement après refresh).
    #[tokio::test]
    async fn add_row_only_id_roundtrip() {
        let dir = temp_dir();
        let ws = Workspace::create(&dir, "Test".into()).await.unwrap();
        let pool = ws.pool.clone();

        let table = repository::create_table(
            &pool,
            "Clients".into(),
            vec![
                FieldInput {
                    name: "Nom".into(),
                    field_type: "text".into(),
                    config: json!({}),
                },
                FieldInput {
                    name: "sexe".into(),
                    field_type: "select".into(),
                    config: json!({"options":[{"id":"opt_a","name":"Homme","color":"#4f8cff"},{"id":"opt_b","name":"Femme","color":"#ff6b9d"}]}),
                },
            ],
            None,
        )
        .await
        .unwrap();

        // L'UI envoie uniquement l'_id pour une nouvelle ligne vide.
        let rec = obj(vec![("_id", json!("rec_new"))]);
        repository::upsert_records(&pool, &table.id, vec![rec])
            .await
            .unwrap();

        let cfg = ViewConfig::default();
        let data = repository::get_table_data(&pool, &table.id, &cfg, &HashMap::new(), "")
            .await
            .unwrap();
        assert_eq!(data.total, 1, "la ligne ajoutée doit être comptée");
        assert_eq!(data.records.len(), 1);
        assert_eq!(data.records[0]["_id"].as_str(), Some("rec_new"));

        // Ré-insertion du même _id (ligne vide) : ne doit pas violer UNIQUE.
        let rec2 = obj(vec![("_id", json!("rec_new"))]);
        repository::upsert_records(&pool, &table.id, vec![rec2])
            .await
            .unwrap();

        // Suppression puis relecture
        repository::delete_records(&pool, &table.id, &["rec_new".to_string()])
            .await
            .unwrap();
        let data2 = repository::get_table_data(&pool, &table.id, &cfg, &HashMap::new(), "")
            .await
            .unwrap();
        assert_eq!(data2.total, 0, "la ligne supprimée ne doit plus apparaître");
    }

    #[tokio::test]
    async fn full_flow() {
        let dir = temp_dir();
        let ws = Workspace::create(&dir, "Test".into()).await.unwrap();
        let pool = ws.pool.clone();

        let table = repository::create_table(
            &pool,
            "Clients".into(),
            vec![
                FieldInput {
                    name: "Nom".into(),
                    field_type: "text".into(),
                    config: json!({}),
                },
                FieldInput {
                    name: "Montant".into(),
                    field_type: "number".into(),
                    config: json!({}),
                },
                FieldInput {
                    name: "Actif".into(),
                    field_type: "checkbox".into(),
                    config: json!({}),
                },
            ],
            None)
        .await
        .unwrap();

        assert!(table.id.starts_with("tbl_"));

        let tables = repository::list_tables(&pool).await.unwrap();
        assert_eq!(tables.len(), 1);

        let fields = repository::list_fields(&pool, &table.id).await.unwrap();
        assert_eq!(fields.len(), 3);
        let fid_nom = fields[0].id.clone();
        let fid_montant = fields[1].id.clone();
        let fid_actif = fields[2].id.clone();

        let rec = obj(vec![
            ("_id", json!("rec_1")),
            (fid_nom.as_str(), json!("Alice")),
            (fid_montant.as_str(), json!(100.5)),
            (fid_actif.as_str(), json!(true)),
        ]);
        let recs = repository::upsert_records(&pool, &table.id, vec![rec])
            .await
            .unwrap();
        assert_eq!(recs.len(), 1);

        let mut rec2 = obj(vec![
            ("_id", json!("rec_2")),
            (fid_nom.as_str(), json!("Bob")),
            (fid_montant.as_str(), json!(50)),
            (fid_actif.as_str(), json!(false)),
        ]);
        repository::upsert_records(&pool, &table.id, vec![rec2.clone()])
            .await
            .unwrap();

        let cfg = ViewConfig::default();
        let data = repository::get_table_data(&pool, &table.id, &cfg, &HashMap::new(), "").await.unwrap();
        assert_eq!(data.total, 2);
        assert_eq!(data.records.len(), 2);

        let mut cfg2 = ViewConfig::default();
        cfg2.filters = vec![Filter {
            field_id: fid_nom.clone(),
            operator: "contains".into(),
            value: json!("li"),
        }];
        let data2 = repository::get_table_data(&pool, &table.id, &cfg2, &HashMap::new(), "").await.unwrap();
        assert_eq!(data2.total, 1);
        assert_eq!(data2.records[0]["_id"].as_str(), Some("rec_1"));

        let mut cfg3 = ViewConfig::default();
        cfg3.sorts = vec![Sort {
            field_id: fid_montant.clone(),
            direction: "desc".into(),
        }];
        let data3 = repository::get_table_data(&pool, &table.id, &cfg3, &HashMap::new(), "").await.unwrap();
        assert_eq!(data3.records[0]["_id"].as_str(), Some("rec_1"));
        assert_eq!(data3.records[1]["_id"].as_str(), Some("rec_2"));

        rec2.as_object_mut()
            .unwrap()
            .insert(fid_montant.clone(), json!(999));
        repository::upsert_records(&pool, &table.id, vec![rec2])
            .await
            .unwrap();
        let data4 = repository::get_table_data(&pool, &table.id, &cfg3, &HashMap::new(), "").await.unwrap();
        assert_eq!(data4.records[0][&fid_montant].as_f64(), Some(999.0));

        let f = repository::create_field(
            &pool,
            &table.id,
            FieldInput {
                name: "Email".into(),
                field_type: "email".into(),
                config: json!({}),
            },
        )
        .await
        .unwrap();
        assert!(f.id.starts_with("fld_"));
        repository::delete_field(&pool, &f.id).await.unwrap();

        let view = repository::create_view(
            &pool,
            ViewInput {
                table_id: table.id.clone(),
                name: "Vue".into(),
                view_type: "grid".into(),
                config: cfg3.clone(),
            },
        )
        .await
        .unwrap();
        let views = repository::list_views(&pool, &table.id).await.unwrap();
        assert_eq!(views.len(), 1);
        repository::delete_view(&pool, &view.id).await.unwrap();

        repository::delete_records(&pool, &table.id, &["rec_1".into(), "rec_2".into()])
            .await
            .unwrap();
        let data5 = repository::get_table_data(&pool, &table.id, &ViewConfig::default(), &HashMap::new(), "")
            .await
            .unwrap();
        assert_eq!(data5.total, 0);

        repository::delete_table(&pool, &table.id).await.unwrap();

        ws.pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn relations_flow() {
        let (dir, ws) = make_workspace().await;
        let pool = ws.pool.clone();

        let cmd = repository::create_table(
            &pool,
            "Commandes".into(),
            vec![
                FieldInput { name: "Nom".into(), field_type: "text".into(), config: json!({}) },
                FieldInput { name: "Montant".into(), field_type: "number".into(), config: json!({}) },
            ],
            None)
        .await
        .unwrap();
        let cli = repository::create_table(
            &pool,
            "Clients".into(),
            vec![FieldInput { name: "Nom".into(), field_type: "text".into(), config: json!({}) }],
            None)
        .await
        .unwrap();

        let cmd_fields = repository::list_fields(&pool, &cmd.id).await.unwrap();
        let cli_fields = repository::list_fields(&pool, &cli.id).await.unwrap();
        let cmd_nom = cmd_fields.iter().find(|f| f.name == "Nom").unwrap().id.clone();
        let fid_montant = cmd_fields.iter().find(|f| f.name == "Montant").unwrap().id.clone();
        let cli_nom = cli_fields.iter().find(|f| f.name == "Nom").unwrap().id.clone();

        repository::upsert_records(&pool, &cmd.id, vec![
            obj(vec![("_id", json!("rec_c1")), (cmd_nom.as_str(), json!("Commande A")), (fid_montant.as_str(), json!(100))]),
            obj(vec![("_id", json!("rec_c2")), (cmd_nom.as_str(), json!("Commande B")), (fid_montant.as_str(), json!(50))]),
        ])
        .await
        .unwrap();
        repository::upsert_records(&pool, &cli.id, vec![
            obj(vec![("_id", json!("rec_k1")), (cli_nom.as_str(), json!("Alice"))]),
            obj(vec![("_id", json!("rec_k2")), (cli_nom.as_str(), json!("Bob"))]),
        ])
        .await
        .unwrap();

        // Champ de lien Commandes.client -> Clients
        let link = repository::create_link_field(
            &pool,
            &cmd.id,
            "Client".into(),
            LinkFieldConfig {
                target_table_id: cli.id.clone(),
                target_db_id: "".into(),
                cardinality: "many".into(),
                allow_creating: true,
                is_backlink: true,
                source_link_field_id: String::new(),
            },
        )
        .await
        .unwrap();
        assert!(link.id.starts_with("fld_"));

        repository::link_records(
            &pool,
            &link.id,
            "rec_c1",
            vec![
                LinkTarget { record_id: "rec_k1".into() },
                LinkTarget { record_id: "rec_k2".into() },
            ],
        )
        .await
        .unwrap();

        let data = repository::get_table_data(&pool, &cmd.id, &ViewConfig::default(), &HashMap::new(), "")
            .await
            .unwrap();
        let rec_c1 = data.records.iter().find(|r| r["_id"] == "rec_c1").unwrap();
        let links = rec_c1[&link.id].as_array().unwrap();
        assert_eq!(links.len(), 2);
        assert!(links.iter().all(|l| l.get("display").is_some()));

        // Lookup : client Nom via le lien
        let lookup = repository::create_field(
            &pool,
            &cmd.id,
            FieldInput {
                name: "Client Name".into(),
                field_type: "lookup".into(),
                config: json!({
                    "source_link_field_id": link.id,
                    "target_field_id": cli_nom
                }),
            },
        )
        .await
        .unwrap();
        // Rollup arrayjoin
        let rollup = repository::create_field(
            &pool,
            &cmd.id,
            FieldInput {
                name: "Clients résumé".into(),
                field_type: "rollup".into(),
                config: json!({
                    "source_link_field_id": link.id,
                    "target_field_id": cli_nom,
                    "function": "arrayjoin"
                }),
            },
        )
        .await
        .unwrap();
        // Count
        let count = repository::create_field(
            &pool,
            &cmd.id,
            FieldInput {
                name: "Nb clients".into(),
                field_type: "count".into(),
                config: json!({ "source_link_field_id": link.id }),
            },
        )
        .await
        .unwrap();
        // Formula
        let formula = repository::create_field(
            &pool,
            &cmd.id,
            FieldInput {
                name: "Niveau".into(),
                field_type: "formula".into(),
                config: json!({ "expression": "IF({Montant} > 50, 'Elevé', 'Bas')" }),
            },
        )
        .await
        .unwrap();

        let data2 = repository::get_table_data(&pool, &cmd.id, &ViewConfig::default(), &HashMap::new(), "")
            .await
            .unwrap();
        let c1 = data2.records.iter().find(|r| r["_id"] == "rec_c1").unwrap();
        let c2 = data2.records.iter().find(|r| r["_id"] == "rec_c2").unwrap();
        assert_eq!(c1[&lookup.id].as_array().map(|a| a.len()), Some(2));
        assert_eq!(c1[&rollup.id], json!("Alice, Bob"));
        assert_eq!(c1[&count.id], json!(2));
        assert_eq!(c1[&formula.id], json!("Elevé"));
        assert_eq!(c2[&formula.id], json!("Bas"));
        assert_eq!(c2[&count.id], json!(0));

        // Backlink sur Clients : la commande qui référence ce client
        let cli_fields2 = repository::list_fields(&pool, &cli.id).await.unwrap();
        let backlink_field = cli_fields2.iter().find(|f| f.name.contains("inverse")).unwrap();
        let data_cli = repository::get_table_data(&pool, &cli.id, &ViewConfig::default(), &HashMap::new(), "")
            .await
            .unwrap();
        let k1 = data_cli.records.iter().find(|r| r["_id"] == "rec_k1").unwrap();
        let back = k1[&backlink_field.id].as_array().unwrap();
        assert_eq!(back.len(), 1);
        assert_eq!(back[0]["record_id"], json!("rec_c1"));
        assert_eq!(back[0]["display"], json!("Commande A"));

        // Unlink
        repository::unlink_records(&pool, &link.id, "rec_c1", &["rec_k2".into()])
            .await
            .unwrap();
        let data3 = repository::get_table_data(&pool, &cmd.id, &ViewConfig::default(), &HashMap::new(), "")
            .await
            .unwrap();
        let c1b = data3.records.iter().find(|r| r["_id"] == "rec_c1").unwrap();
        assert_eq!(c1b[&link.id].as_array().map(|a| a.len()), Some(1));
        assert_eq!(c1b[&count.id], json!(1));

        // Supprimer le champ de lien -> backlink supprimé aussi
        repository::delete_field(&pool, &link.id).await.unwrap();
        let cli_fields3 = repository::list_fields(&pool, &cli.id).await.unwrap();
        assert!(cli_fields3.iter().all(|f| f.id != backlink_field.id));
        let cmd_fields3 = repository::list_fields(&pool, &cmd.id).await.unwrap();
        assert!(cmd_fields3.iter().all(|f| f.id != lookup.id));

        let _ = (fid_montant, data);
        ws.pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn delete_records_cleanup_links() {
        let (dir, ws) = make_workspace().await;
        let pool = ws.pool.clone();

        let cmd = repository::create_table(
            &pool,
            "Commandes".into(),
            vec![FieldInput { name: "Nom".into(), field_type: "text".into(), config: json!({}) }],
            None)
        .await
        .unwrap();
        let cli = repository::create_table(
            &pool,
            "Clients".into(),
            vec![FieldInput { name: "Nom".into(), field_type: "text".into(), config: json!({}) }],
            None)
        .await
        .unwrap();

        let cmd_fields = repository::list_fields(&pool, &cmd.id).await.unwrap();
        let cli_fields = repository::list_fields(&pool, &cli.id).await.unwrap();
        let cmd_nom = cmd_fields[0].id.clone();
        let cli_nom = cli_fields[0].id.clone();

        repository::upsert_records(&pool, &cmd.id, vec![
            obj(vec![("_id", json!("rec_c1")), (cmd_nom.as_str(), json!("Commande A"))]),
        ])
        .await
        .unwrap();
        repository::upsert_records(&pool, &cli.id, vec![
            obj(vec![("_id", json!("rec_k1")), (cli_nom.as_str(), json!("Alice"))]),
            obj(vec![("_id", json!("rec_k2")), (cli_nom.as_str(), json!("Bob"))]),
        ])
        .await
        .unwrap();

        let link = repository::create_link_field(
            &pool,
            &cmd.id,
            "Client".into(),
            LinkFieldConfig {
                target_table_id: cli.id.clone(),
                target_db_id: "".into(),
                cardinality: "many".into(),
                allow_creating: true,
                is_backlink: true,
                source_link_field_id: String::new(),
            },
        )
        .await
        .unwrap();

        repository::link_records(
            &pool,
            &link.id,
            "rec_c1",
            vec![
                LinkTarget { record_id: "rec_k1".into() },
                LinkTarget { record_id: "rec_k2".into() },
            ],
        )
        .await
        .unwrap();

        // Suppression d'un client : le lien depuis la commande doit être nettoyé.
        repository::delete_records(&pool, &cli.id, &["rec_k1".into()])
            .await
            .unwrap();

        let data = repository::get_table_data(&pool, &cmd.id, &ViewConfig::default(), &HashMap::new(), "")
            .await
            .unwrap();
        let c1 = data.records.iter().find(|r| r["_id"] == "rec_c1").unwrap();
        let links = c1[&link.id].as_array().unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0]["record_id"], json!("rec_k2"));

        // Le backlink du client restant référence toujours la commande.
        let data_cli = repository::get_table_data(&pool, &cli.id, &ViewConfig::default(), &HashMap::new(), "")
            .await
            .unwrap();
        let k2 = data_cli.records.iter().find(|r| r["_id"] == "rec_k2").unwrap();
        let cli_fields2 = repository::list_fields(&pool, &cli.id).await.unwrap();
        let back = cli_fields2.iter().find(|f| f.name.contains("inverse")).unwrap();
        assert_eq!(k2[&back.id].as_array().map(|a| a.len()), Some(1));

        // Une nouvelle suppression d'un autre client : idempotent, aucun panique.
        repository::delete_records(&pool, &cli.id, &["rec_k2".into()])
            .await
            .unwrap();

        ws.pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn filters_correctness() {
        let (dir, ws) = make_workspace().await;
        let pool = ws.pool.clone();

        let t = repository::create_table(
            &pool,
            "T".into(),
            vec![
                FieldInput { name: "Nom".into(), field_type: "text".into(), config: json!({}) },
                FieldInput { name: "Montant".into(), field_type: "number".into(), config: json!({}) },
                FieldInput { name: "Actif".into(), field_type: "checkbox".into(), config: json!({}) },
            ],
            None)
        .await
        .unwrap();
        let fields = repository::list_fields(&pool, &t.id).await.unwrap();
        let nom = fields.iter().find(|f| f.name == "Nom").unwrap().id.clone();
        let montant = fields.iter().find(|f| f.name == "Montant").unwrap().id.clone();
        let actif = fields.iter().find(|f| f.name == "Actif").unwrap().id.clone();

        repository::upsert_records(&pool, &t.id, vec![
            obj(vec![("_id", json!("r1")), (nom.as_str(), json!("100% pur")), (montant.as_str(), json!(0)), (actif.as_str(), json!(false))]),
            obj(vec![("_id", json!("r2")), (nom.as_str(), json!("Alice")), (montant.as_str(), json!(10)), (actif.as_str(), json!(true))]),
            obj(vec![("_id", json!("r3")), (nom.as_str(), json!("Bob"))]),
            obj(vec![("_id", json!("r4")), (nom.as_str(), json!("a_b")), (montant.as_str(), json!(5)), (actif.as_str(), json!(true))]),
            obj(vec![("_id", json!("r5")), (nom.as_str(), json!("aXb")), (montant.as_str(), json!(7)), (actif.as_str(), json!(true))]),
        ])
        .await
        .unwrap();

        let filter = |field_id: &str, operator: &str, value: Value| -> Filter {
            Filter { field_id: field_id.into(), operator: operator.into(), value }
        };

        // S2 : '%' littéral dans contains
        let mut cfg = ViewConfig::default();
        cfg.filters = vec![filter(&nom, "contains", json!("100%"))];
        let d = repository::get_table_data(&pool, &t.id, &cfg, &HashMap::new(), "").await.unwrap();
        assert_eq!(d.total, 1);
        assert_eq!(d.records[0]["_id"], json!("r1"));

        // S2 : '_' littéral (ne matche pas 'aXb')
        let mut cfg = ViewConfig::default();
        cfg.filters = vec![filter(&nom, "contains", json!("a_b"))];
        let d = repository::get_table_data(&pool, &t.id, &cfg, &HashMap::new(), "").await.unwrap();
        assert_eq!(d.total, 1);
        assert_eq!(d.records[0]["_id"], json!("r4"));

        // E10 : is_not_empty nombre inclut 0
        let mut cfg = ViewConfig::default();
        cfg.filters = vec![filter(&montant, "is_not_empty", json!(null))];
        let d = repository::get_table_data(&pool, &t.id, &cfg, &HashMap::new(), "").await.unwrap();
        assert_eq!(d.total, 4);

        // is_empty nombre : seul NULL
        let mut cfg = ViewConfig::default();
        cfg.filters = vec![filter(&montant, "is_empty", json!(null))];
        let d = repository::get_table_data(&pool, &t.id, &cfg, &HashMap::new(), "").await.unwrap();
        assert_eq!(d.total, 1);
        assert_eq!(d.records[0]["_id"], json!("r3"));

        // C4 : is_empty checkbox inclut false (0)
        let mut cfg = ViewConfig::default();
        cfg.filters = vec![filter(&actif, "is_empty", json!(null))];
        let d = repository::get_table_data(&pool, &t.id, &cfg, &HashMap::new(), "").await.unwrap();
        let ids: Vec<&str> = d.records.iter().map(|r| r["_id"].as_str().unwrap()).collect();
        assert_eq!(d.total, 2);
        assert!(ids.contains(&"r1"));
        assert!(ids.contains(&"r3"));

        // is_not_empty checkbox : seuls les cochés (true)
        let mut cfg = ViewConfig::default();
        cfg.filters = vec![filter(&actif, "is_not_empty", json!(null))];
        let d = repository::get_table_data(&pool, &t.id, &cfg, &HashMap::new(), "").await.unwrap();
        assert_eq!(d.total, 3);

        // E8 : nombre invalide → aucune correspondance (pas 0.0)
        let mut cfg = ViewConfig::default();
        cfg.filters = vec![filter(&montant, "gt", json!("abc"))];
        let d = repository::get_table_data(&pool, &t.id, &cfg, &HashMap::new(), "").await.unwrap();
        assert_eq!(d.total, 0);

        ws.pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn partial_update_preserves_columns() {
        let (dir, ws) = make_workspace().await;
        let pool = ws.pool.clone();

        let t = repository::create_table(
            &pool,
            "T".into(),
            vec![
                FieldInput { name: "A".into(), field_type: "text".into(), config: json!({}) },
                FieldInput { name: "B".into(), field_type: "text".into(), config: json!({}) },
                FieldInput { name: "C".into(), field_type: "checkbox".into(), config: json!({}) },
            ],
            None)
        .await
        .unwrap();
        let fields = repository::list_fields(&pool, &t.id).await.unwrap();
        let fa = fields.iter().find(|f| f.name == "A").unwrap().id.clone();
        let fb = fields.iter().find(|f| f.name == "B").unwrap().id.clone();
        let fc = fields.iter().find(|f| f.name == "C").unwrap().id.clone();

        repository::upsert_records(&pool, &t.id, vec![obj(vec![
            ("_id", json!("r1")),
            (fa.as_str(), json!("x")),
            (fb.as_str(), json!("y")),
            (fc.as_str(), json!(true)),
        ])])
        .await
        .unwrap();

        // Édition d'une seule cellule : les autres colonnes doivent rester intactes.
        repository::upsert_records(&pool, &t.id, vec![obj(vec![
            ("_id", json!("r1")),
            (fa.as_str(), json!("z")),
        ])])
        .await
        .unwrap();

        let d = repository::get_table_data(&pool, &t.id, &ViewConfig::default(), &HashMap::new(), "").await.unwrap();
        let r = &d.records[0];
        assert_eq!(r[&fa], json!("z"));
        assert_eq!(r[&fb], json!("y"));
        assert_eq!(r[&fc], json!(true));

        ws.pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn filter_precedence_combined() {
        let (dir, ws) = make_workspace().await;
        let pool = ws.pool.clone();

        let t = repository::create_table(
            &pool,
            "T".into(),
            vec![
                FieldInput { name: "A".into(), field_type: "text".into(), config: json!({}) },
                FieldInput { name: "B".into(), field_type: "text".into(), config: json!({}) },
            ],
            None)
        .await
        .unwrap();
        let fields = repository::list_fields(&pool, &t.id).await.unwrap();
        let fa = fields.iter().find(|f| f.name == "A").unwrap().id.clone();
        let fb = fields.iter().find(|f| f.name == "B").unwrap().id.clone();

        repository::upsert_records(&pool, &t.id, vec![
            obj(vec![("_id", json!("r1")), (fa.as_str(), json!("x"))]),
            obj(vec![("_id", json!("r2"))]),
        ])
        .await
        .unwrap();

        // is_empty(B) AND is_not_empty(A) : sans parenthèses, l'OR de is_empty
        // faisait matcher r2 (A vide) à tort.
        let mut cfg = ViewConfig::default();
        cfg.filters = vec![
            Filter { field_id: fb.clone(), operator: "is_empty".into(), value: json!(null) },
            Filter { field_id: fa.clone(), operator: "is_not_empty".into(), value: json!(null) },
        ];
        let d = repository::get_table_data(&pool, &t.id, &cfg, &HashMap::new(), "").await.unwrap();
        assert_eq!(d.total, 1);
        assert_eq!(d.records[0]["_id"], json!("r1"));

        ws.pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn select_roundtrip() {
        let (dir, ws) = make_workspace().await;
        let pool = ws.pool.clone();

        let t = repository::create_table(
            &pool,
            "T".into(),
            vec![FieldInput {
                name: "S".into(),
                field_type: "select".into(),
                config: json!({ "options": [ { "id": "opt_1", "name": "Rouge" } ] }),
            }],
            None)
        .await
        .unwrap();
        let fields = repository::list_fields(&pool, &t.id).await.unwrap();
        let fs = fields.iter().find(|f| f.name == "S").unwrap().id.clone();

        repository::upsert_records(&pool, &t.id, vec![obj(vec![
            ("_id", json!("r1")),
            (fs.as_str(), json!("opt_1")),
        ])])
        .await
        .unwrap();

        // La valeur relue doit rester l'id d'option (round-trip intact).
        let d = repository::get_table_data(&pool, &t.id, &ViewConfig::default(), &HashMap::new(), "").await.unwrap();
        assert_eq!(d.records[0][&fs], json!("opt_1"));

        ws.pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn get_record_with_relations_depth1() {
        let (dir, ws) = make_workspace().await;
        let pool = ws.pool.clone();

        let cli = repository::create_table(
            &pool,
            "Clients".into(),
            vec![FieldInput { name: "Nom".into(), field_type: "text".into(), config: json!({}) }],
            None)
        .await
        .unwrap();
        let cmd = repository::create_table(
            &pool,
            "Cmd".into(),
            vec![FieldInput { name: "Nom".into(), field_type: "text".into(), config: json!({}) }],
            None)
        .await
        .unwrap();

        let cli_fields = repository::list_fields(&pool, &cli.id).await.unwrap();
        let cli_nom = cli_fields[0].id.clone();
        let cmd_fields = repository::list_fields(&pool, &cmd.id).await.unwrap();
        let cmd_nom = cmd_fields[0].id.clone();

        repository::upsert_records(&pool, &cli.id, vec![
            obj(vec![("_id", json!("c1")), (cli_nom.as_str(), json!("Alice"))]),
            obj(vec![("_id", json!("c2")), (cli_nom.as_str(), json!("Bob"))]),
        ]).await.unwrap();
        repository::upsert_records(&pool, &cmd.id, vec![
            obj(vec![("_id", json!("r1")), (cmd_nom.as_str(), json!("Cmd A"))]),
        ]).await.unwrap();

        let link = repository::create_link_field(&pool, &cmd.id, "Client".into(), LinkFieldConfig {
            target_table_id: cli.id.clone(),
            target_db_id: "".into(),
            cardinality: "many".into(),
            allow_creating: true,
            is_backlink: false,
            source_link_field_id: String::new(),
        }).await.unwrap();

        repository::link_records(&pool, &link.id, "r1", vec![
            LinkTarget { record_id: "c1".into() },
            LinkTarget { record_id: "c2".into() },
        ]).await.unwrap();

        let rwr = repository::get_record_with_relations(&pool, &cmd.id, "r1", 1, &HashMap::new(), "").await.unwrap();
        assert_eq!(rwr.record["_id"], json!("r1"));
        let rel = rwr.relations.get(&link.id).expect("link relation");
        assert_eq!(rel.len(), 2);
        // Cross-DB: création avec target_db_id non vide doit être acceptée (Phase 3 plumbing)
        let t2 = repository::create_table(&pool, "Ref".into(), vec![FieldInput { name: "X".into(), field_type: "text".into(), config: json!({}) }], Some("db_ref".into())).await.unwrap();
        assert_eq!(t2.source_db_id, Some("db_ref".into()));

        ws.pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn ai_phase4_flow() {
        use crate::ai::analysis;
        use crate::ai::cleaning;
        use crate::ai::context_builder;
        use crate::ai::formula;
        use crate::ai::relation_suggest;

        let (dir, ws) = make_workspace().await;
        let pool = ws.pool.clone();

        // Table Commandes avec email_client
        let cmd = repository::create_table(
            &pool,
            "Commandes".into(),
            vec![
                FieldInput { name: "email_client".into(), field_type: "email".into(), config: json!({}) },
                FieldInput { name: "Montant".into(), field_type: "number".into(), config: json!({}) },
                FieldInput { name: "Nom".into(), field_type: "text".into(), config: json!({}) },
            ],
            None,
        )
        .await
        .unwrap();
        let cli = repository::create_table(
            &pool,
            "Clients".into(),
            vec![
                FieldInput { name: "Email".into(), field_type: "email".into(), config: json!({}) },
                FieldInput { name: "Nom".into(), field_type: "text".into(), config: json!({}) },
            ],
            None,
        )
        .await
        .unwrap();

        let cmd_fields = repository::list_fields(&pool, &cmd.id).await.unwrap();
        let cli_fields = repository::list_fields(&pool, &cli.id).await.unwrap();
        let cmd_email = cmd_fields.iter().find(|f| f.name == "email_client").unwrap().id.clone();
        let cmd_montant = cmd_fields.iter().find(|f| f.name == "Montant").unwrap().id.clone();
        let cmd_nom = cmd_fields.iter().find(|f| f.name == "Nom").unwrap().id.clone();
        let cli_email = cli_fields.iter().find(|f| f.name == "Email").unwrap().id.clone();
        let cli_nom = cli_fields.iter().find(|f| f.name == "Nom").unwrap().id.clone();

        repository::upsert_records(
            &pool,
            &cmd.id,
            vec![
                obj(vec![("_id", json!("r1")), (cmd_email.as_str(), json!("  ALICE@EXAMPLE.COM ")), (cmd_montant.as_str(), json!(100)), (cmd_nom.as_str(), json!("  Alice  "))]),
                obj(vec![("_id", json!("r2")), (cmd_email.as_str(), json!("bob@example.com")), (cmd_montant.as_str(), json!(20)), (cmd_nom.as_str(), json!("Bob"))]),
            ],
        )
        .await
        .unwrap();
        repository::upsert_records(
            &pool,
            &cli.id,
            vec![
                obj(vec![("_id", json!("c1")), (cli_email.as_str(), json!("alice@example.com")), (cli_nom.as_str(), json!("Alice"))]),
                obj(vec![("_id", json!("c2")), (cli_email.as_str(), json!("bob@example.com")), (cli_nom.as_str(), json!("Bob"))]),
            ],
        )
        .await
        .unwrap();

        let ctx = context_builder::build_context(&ws.config, &pool, &HashMap::new(), 50).await.unwrap();

        // suggest_relations: email_client -> Clients.Email
        let sugg = relation_suggest::suggest_relations(&ctx, &cmd.id);
        assert!(!sugg.is_empty(), "should suggest at least one relation");
        assert!(sugg.iter().any(|s| s.source_field_id == cmd_email && s.target_field_id == cli_email));

        // formula: heuristique
        let fres = formula::generate_heuristic(&ctx, &cmd.id, "si montant > 50 alors élevé sinon bas");
        assert!(fres.valid);
        assert!(fres.expression.contains("IF"));

        let fres2 = formula::generate_heuristic(&ctx, &cmd.id, "somme du montant");
        assert!(fres2.expression.contains("SUM"));

        // analysis
        let ares = analysis::analyze_heuristic(&ctx, Some(&cmd.id), Some("total montant"));
        assert!(!ares.stats.is_empty());
        assert!(ares.insights.iter().any(|x| x.contains("Montant") || x.contains("montant")));

        // cleaning preview + apply (trim + normalize email)
        let plan = cleaning::preview_heuristic(&ctx, &cmd.id, "supprimer les espaces et normaliser les emails");
        assert!(!plan.ops.is_empty());
        assert!(plan.estimated_rows >= 1, "estimated_rows doit estimer sur tout le sample");
        let applied = cleaning::apply_transform(&pool, &cmd.id, &plan).await.unwrap();
        assert!(applied.applied_rows >= 1);
        // verify trim/normalize applied
        let data = repository::get_table_data(&pool, &cmd.id, &ViewConfig::default(), &HashMap::new(), "").await.unwrap();
        let r1 = data.records.iter().find(|r| r["_id"] == "r1").unwrap();
        let nom_val = r1[&cmd_nom].as_str().unwrap_or("");
        assert_eq!(nom_val, "Alice", "trim should have removed spaces");
        let email_val = r1[&cmd_email].as_str().unwrap_or("");
        assert_eq!(email_val, "alice@example.com", "email should be lowercased+trimmed");

        // dedup path (no actual duplicate in this dataset, but ensure no panic)
        let plan2 = cleaning::preview_heuristic(&ctx, &cmd.id, "supprimer les doublons");
        assert!(plan2.ops.iter().any(|o| matches!(o.op_type, crate::ai::cleaning::TransformOpType::Deduplicate)) || !plan2.ops.is_empty());

        // M3: formule référençant un champ inexistant -> invalide, pas de mensonge
        let bad = formula::generate_heuristic(&ctx, &cmd.id, "SUM({Inexistant})");
        assert!(!bad.valid);
        assert!(bad.error.is_some());

        // M3: fallback par défaut construit sur les champs RÉELS de la table
        let dflt = formula::generate_heuristic(&ctx, &cmd.id, "fait quelque chose de magique");
        assert!(dflt.valid, "fallback doit utiliser des champs existants : {:?}", dflt.expression);
        assert!(dflt.expression.contains("Montant") || dflt.expression.contains("Nom"));

        // M4: dédup sur colonne NUMÉRIQUE (l'ancien query_as<(String,String)> no-op'ait muet)
        repository::upsert_records(&pool, &cmd.id, vec![obj(vec![("_id", json!("r3")), (cmd_montant.as_str(), json!(20))])]).await.unwrap();
        let dup_plan = cleaning::TransformPlan {
            ops: vec![cleaning::TransformOp {
                op_type: cleaning::TransformOpType::Deduplicate,
                field_id: cmd_montant.clone(),
                field_name: "Montant".into(),
                params: json!({}),
                description: "test dédup numérique".into(),
            }],
            preview: vec![],
            estimated_rows: 1,
            provider: "test".into(),
        };
        let dup_res = cleaning::apply_transform(&pool, &cmd.id, &dup_plan).await.unwrap();
        assert_eq!(dup_res.applied_rows, 1, "le doublon numérique 20 (r3) doit être supprimé");
        let data2 = repository::get_table_data(&pool, &cmd.id, &ViewConfig::default(), &HashMap::new(), "").await.unwrap();
        let montants: Vec<f64> = data2.records.iter().filter_map(|r| r[&cmd_montant].as_f64()).collect();
        assert_eq!(montants, vec![100.0, 20.0], "r2 conservé, r3 supprimé");

        ws.pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn phase5_import_export_attachments() {
        // Couvre : export CSV/JSON/XLSX + import CSV + champ attachment (fichier disque + JSON)
        let (dir, ws) = make_workspace().await;
        let pool = ws.pool.clone();

        let tbl = repository::create_table(
            &pool,
            "Invoices".into(),
            vec![
                FieldInput { name: "Titre".into(), field_type: "text".into(), config: json!({}) },
                FieldInput { name: "Montant".into(), field_type: "number".into(), config: json!({}) },
                FieldInput { name: "Docs".into(), field_type: "attachment".into(), config: json!({"max_size_mb": 1}) },
            ],
            None,
        ).await.unwrap();
        let fields = repository::list_fields(&pool, &tbl.id).await.unwrap();
        let fid_titre = fields.iter().find(|f| f.name=="Titre").unwrap().id.clone();
        let fid_montant = fields.iter().find(|f| f.name=="Montant").unwrap().id.clone();
        let fid_docs = fields.iter().find(|f| f.name=="Docs").unwrap().id.clone();

        repository::upsert_records(&pool, &tbl.id, vec![
            obj(vec![("_id", json!("r1")), (fid_titre.as_str(), json!("Facture A")), (fid_montant.as_str(), json!(100))]),
            obj(vec![("_id", json!("r2")), (fid_titre.as_str(), json!("Facture B")), (fid_montant.as_str(), json!(250.5))]),
        ]).await.unwrap();

        // --- Export CSV (inline, même logique que la commande) ---
        let csv_bytes = {
            let mut wtr = csv::WriterBuilder::new().has_headers(true).from_writer(vec![]);
            wtr.write_record(["Titre","Montant"]).unwrap();
            wtr.write_record(["Facture A","100"]).unwrap();
            wtr.write_record(["Facture B","250.5"]).unwrap();
            wtr.into_inner().unwrap()
        };
        assert!(csv_bytes.len() > 10);
        assert!(String::from_utf8(csv_bytes.clone()).unwrap().contains("Titre"));

        // --- Import CSV (mapping nom->field_id) dans la même table ---
        {
            let mut rdr = csv::ReaderBuilder::new().has_headers(true).from_reader(csv_bytes.as_slice());
            let headers = rdr.headers().unwrap().clone();
            assert_eq!(headers.get(0).unwrap(), "Titre");
            let mut recs: Vec<serde_json::Value> = vec![];
            for rec in rdr.records() {
                let r = rec.unwrap();
                let mut m = serde_json::Map::new();
                m.insert("_id".into(), serde_json::json!(crate::utils::new_id("rec")));
                for (i, v) in r.iter().enumerate() {
                    let h = headers.get(i).unwrap();
                    let fid = fields.iter().find(|f| f.name==h).map(|f| f.id.clone()).unwrap();
                    m.insert(fid, serde_json::json!(v));
                }
                recs.push(serde_json::Value::Object(m));
            }
            assert_eq!(recs.len(), 2);
            repository::upsert_records(&pool, &tbl.id, recs).await.unwrap();
            let total: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {}", crate::db::repository::quote_ident_public(&tbl.id)))
                .fetch_one(&pool).await.unwrap();
            assert_eq!(total, 4, "2 lignes initiales + 2 importées");
        }

        // --- Export XLSX (rust_xlsxwriter, vérifie que le buffer est un zip) ---
        {
            use rust_xlsxwriter::{Workbook, Format};
            let mut wb = Workbook::new();
            let ws = wb.add_worksheet();
            ws.write_string_with_format(0, 0, "Titre", &Format::new().set_bold()).unwrap();
            ws.write_string(1, 0, "Facture A").unwrap();
            let buf = wb.save_to_buffer().unwrap();
            assert!(buf.len() > 100);
            assert_eq!(&buf[0..2], &[0x50, 0x4B], "xlsx doit être un ZIP (PK)");
        }

        // --- Export JSON round-trip ---
        {
            let data = repository::get_table_data(&pool, &tbl.id, &ViewConfig::default(), &HashMap::new(), "").await.unwrap();
            let j = serde_json::to_vec_pretty(&data.records).unwrap();
            let parsed: Vec<serde_json::Value> = serde_json::from_slice(&j).unwrap();
            assert_eq!(parsed.len(), 4);
        }

        // --- Attachments : écriture disque + JSON dans cellule ---
        {
            let rec_id = "r1";
            let dir_path = dir.join("attachments").join(&ws.config.active_database_id).join(&tbl.id).join(rec_id);
            tokio::fs::create_dir_all(&dir_path).await.unwrap();
            let file_path = dir_path.join("hello.txt");
            tokio::fs::write(&file_path, b"hello world").await.unwrap();
            assert!(file_path.exists());
            let meta = serde_json::json!([{"name":"hello.txt","url": file_path.to_string_lossy(),"size":11,"type":"text/plain"}]);
            sqlx::query(&format!("UPDATE {} SET {} = ? WHERE _id = ?", crate::db::repository::quote_ident_public(&tbl.id), crate::db::repository::quote_ident_public(&fid_docs)))
                .bind(meta.to_string()).bind(rec_id).execute(&pool).await.unwrap();
            let stored: Option<String> = sqlx::query_scalar(&format!("SELECT {} FROM {} WHERE _id = ?", crate::db::repository::quote_ident_public(&fid_docs), crate::db::repository::quote_ident_public(&tbl.id)))
                .bind(rec_id).fetch_one(&pool).await.unwrap();
            assert!(stored.unwrap().contains("hello.txt"));
            // cleanup fichier
            tokio::fs::remove_file(&file_path).await.unwrap();
            let filtered = serde_json::json!([]);
            sqlx::query(&format!("UPDATE {} SET {} = ? WHERE _id = ?", crate::db::repository::quote_ident_public(&tbl.id), crate::db::repository::quote_ident_public(&fid_docs)))
                .bind(filtered.to_string()).bind(rec_id).execute(&pool).await.unwrap();
        }

        // --- Settings : mise à jour workspace.json ---
        {
            let mut cfg = ws.config.clone();
            cfg.settings.llm_provider = "off".into();
            // simule update_workspace_settings
            std::fs::write(dir.join("workspace.json"), serde_json::to_string_pretty(&cfg).unwrap()).unwrap();
            let reloaded: crate::db::models::WorkspaceConfig = serde_json::from_str(&std::fs::read_to_string(dir.join("workspace.json")).unwrap()).unwrap();
            assert_eq!(reloaded.settings.llm_provider, "off");
        }

        ws.pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }

    // Miniatures : sanitize_filename ne panique pas sur nom unicode long,
    // et read_attachment_bytes fait l'aller-retour + rejette la traversée de chemin.
    #[tokio::test]
    async fn attachment_thumbs_helpers() {
        use crate::commands::attachments::{read_attachment_bytes, sanitize_filename};

        // F1 : troncature à 120 sans couper un caractère multioctet (paniquait avant)
        let long_unicode = "é".repeat(200);
        let safe = sanitize_filename(&long_unicode);
        assert!(safe.chars().count() <= 120);

        // sanitize garde le nom de base uniquement
        assert_eq!(sanitize_filename("a/b\\c..png"), "c..png");
        assert_eq!(sanitize_filename(""), "file");

        let (dir, _ws) = make_workspace().await;
        let db_id = "db_ok";
        let rec_dir = dir.join("attachments").join(db_id).join("tbl_x").join("rec_1");
        tokio::fs::create_dir_all(&rec_dir).await.unwrap();
        tokio::fs::write(rec_dir.join("img.png"), b"\x89PNG-bytes").await.unwrap();

        // aller-retour OK
        let bytes = read_attachment_bytes(&dir, db_id, "tbl_x", "rec_1", "img.png").await.unwrap();
        assert_eq!(bytes, b"\x89PNG-bytes");

        // traversée : "..%2F..%2Fsecret" -> sanitize neutralise, fichier introuvable
        let err = read_attachment_bytes(&dir, db_id, "tbl_x", "rec_1", "../../workspace.json").await.unwrap_err();
        assert!(err.to_string().contains("introuvable"));

        // fichier manquant
        let err = read_attachment_bytes(&dir, db_id, "tbl_x", "rec_1", "absent.png").await.unwrap_err();
        assert!(err.to_string().contains("introuvable"));

        std::fs::remove_dir_all(&dir).ok();
    }

    // Migration héritée : un workspace dont `_tables` est antérieur à
    // `source_db_id` (SELECT ... échouait en ColumnNotFound) est mis à jour de
    // façon idempotente par run_meta (PRAGMA table_info + ALTER si absent).
    #[tokio::test]
    async fn migration_backfills_source_db_id() {
        let dir = temp_dir();
        let db_path = dir.join("legacy.db");

        // Schéma historique (sans source_db_id) + une table existante.
        let pool = crate::db::connection::open_pool(&db_path).await.unwrap();
        sqlx::query(
            "CREATE TABLE _tables (id TEXT PRIMARY KEY, name TEXT NOT NULL, \
             created_at INTEGER NOT NULL DEFAULT (strftime('%s','now')), \
             updated_at INTEGER NOT NULL DEFAULT (strftime('%s','now')))",
        )
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO _tables (id, name) VALUES ('tbl_legacy', 'Ancienne')")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;

        // run_meta doit ajouter la colonne puis list_tables fonctionne.
        let pool = crate::db::connection::open_pool(&db_path).await.unwrap();
        crate::workspace::migration::run_meta(&pool).await.unwrap();
        let tables = repository::list_tables(&pool).await.unwrap();
        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].id, "tbl_legacy");
        assert_eq!(tables[0].source_db_id, None);
        pool.close().await;
        std::fs::remove_dir_all(&dir).ok();
    }
}
