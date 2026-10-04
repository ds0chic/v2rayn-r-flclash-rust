use std::path::Path;
use application::BackupService;
use persistence::Store;
fn source(path: &Path, id: &str, theme: &str) {
    std::fs::create_dir_all(path.join("config")).unwrap();
    let store=Store::create(path.join("guiNDB.db")).unwrap();
    for table in persistence::UPSTREAM_TABLES { store.connection().execute_batch(&table.create_sql()).unwrap(); }
    store.connection().execute("INSERT INTO ProfileItem (IndexId,ConfigType,ConfigVersion,Subid,Remarks,Address,Security,Id) VALUES (?1,2,4,'',?1,'custom.json','','')",[id]).unwrap();
    drop(store);
    std::fs::write(path.join("guiNConfig.json"),format!(r#"{{"IndexId":"{id}","SubIndexId":"","UIItem":{{"CurrentTheme":"{theme}"}}}}"#)).unwrap();
    std::fs::write(path.join("config/custom.json"),r#"{"outbounds":[{"protocol":"freedom"}]}"#).unwrap();
}
fn main() {
    let root=std::env::args().nth(1).unwrap();
    let root=Path::new(&root);
    let a=root.join("a"); let b=root.join("b"); let live=root.join("live");
    source(&a,"node-a","Dark"); source(&b,"node-b","Light"); std::fs::create_dir_all(&live).unwrap();
    let service=BackupService::new(&live);
    let ra=service.import_upstream(&a,&root.join("work-a"),1).unwrap();
    let rb=service.import_upstream(&b,&root.join("work-b"),2).unwrap();
    let merged=Store::open(live.join("guiNDB.db")).unwrap().count_rows("ProfileItem").unwrap();
    println!("A={:?}; B={:?}; rows_after_B={}",ra.status,rb.status,merged);
    let again=service.import_upstream(&a,&root.join("work-a-again"),3).unwrap();
    let raw=std::fs::read_to_string(live.join("guiNConfig.json")).unwrap();
    let doc=persistence::ConfigDocument::parse(&raw).unwrap().to_json_value();
    let active=doc.get("active_index_id").and_then(|x|x.as_str()).unwrap();
    let theme=doc.get("UIItem").unwrap().get("CurrentTheme").and_then(|x|x.as_str()).unwrap();
    let store=Store::open(live.join("guiNDB.db")).unwrap();
    let exists:i64=store.connection().query_row("SELECT COUNT(*) FROM ProfileItem WHERE IndexId=?1",[active],|r|r.get(0)).unwrap();
    println!("A_again={:?}; theme_after_A_again={}; active_exists={}",again.status,theme,exists);
    assert_eq!(merged,2,"observed candidate import merges two sources");
    assert_eq!(theme,"Light","observed A reimport incorrectly activates B settings");
    assert_eq!(exists,0,"observed active ID from wrong source is dangling");
}
