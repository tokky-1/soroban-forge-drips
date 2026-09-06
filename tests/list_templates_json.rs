use std::process::Command;

#[test]
fn list_templates_json_valid() {
    let output = Command::new(env!("CARGO_BIN_EXE_soroban-forge"))
        .args(["new", "--list-templates", "--json"])
        .output()
        .expect("failed to run soroban-forge");

    assert!(output.status.success(), "{:?}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("stdout is not UTF-8");
    
    // Parse the JSON to verify it's valid
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .expect("output is not valid JSON");
    
    // Verify it's an array
    assert!(json.is_array(), "JSON output should be an array");
    
    let templates = json.as_array().unwrap();
    assert!(!templates.is_empty(), "should have at least one template");
    
    // Verify each template has required fields
    for template in templates {
        assert!(template.is_object(), "each template should be an object");
        let obj = template.as_object().unwrap();
        
        assert!(obj.contains_key("name"), "template should have 'name' field");
        assert!(obj.contains_key("description"), "template should have 'description' field");
        assert!(obj.contains_key("variables"), "template should have 'variables' field");
        
        assert!(obj["name"].is_string(), "'name' should be a string");
        assert!(obj["description"].is_string(), "'description' should be a string");
        assert!(obj["variables"].is_array(), "'variables' should be an array");
    }
}

#[test]
fn list_templates_json_contains_hello_world() {
    let output = Command::new(env!("CARGO_BIN_EXE_soroban-forge"))
        .args(["new", "--list-templates", "--json"])
        .output()
        .expect("failed to run soroban-forge");

    assert!(output.status.success(), "{:?}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("stdout is not UTF-8");
    
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .expect("output is not valid JSON");
    
    let templates = json.as_array().unwrap();
    let hello_world = templates
        .iter()
        .find(|t| t["name"].as_str() == Some("hello-world"))
        .expect("hello-world template should be in the list");
    
    assert!(hello_world["description"].as_str().unwrap().len() > 0);
    assert!(hello_world["variables"].is_array());
}

#[test]
fn templates_command_json_valid() {
    let output = Command::new(env!("CARGO_BIN_EXE_soroban-forge"))
        .args(["--json", "templates"])
        .output()
        .expect("failed to run soroban-forge");

    assert!(output.status.success(), "{:?}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("stdout is not UTF-8");
    
    // Parse the JSON to verify it's valid
    let json: serde_json::Value = serde_json::from_str(&stdout)
        .expect("output is not valid JSON");
    
    // Verify it's an array
    assert!(json.is_array(), "JSON output should be an array");
    
    let templates = json.as_array().unwrap();
    assert!(!templates.is_empty(), "should have at least one template");
}

#[test]
fn human_readable_listing_unchanged() {
    let output = Command::new(env!("CARGO_BIN_EXE_soroban-forge"))
        .args(["new", "--list-templates"])
        .output()
        .expect("failed to run soroban-forge");

    assert!(output.status.success(), "{:?}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).expect("stdout is not UTF-8");
    
    // Verify it contains expected template names
    assert!(stdout.contains("hello-world"), "should list hello-world template");
    assert!(stdout.contains("token"), "should list token template");
    
    // Verify it's not JSON (i.e., doesn't start with '[' or '{')
    assert!(!stdout.trim().starts_with('['), "should not output JSON in text mode");
    assert!(!stdout.trim().starts_with('{'), "should not output JSON in text mode");
}
