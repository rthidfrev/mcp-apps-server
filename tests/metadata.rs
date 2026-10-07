#![forbid(unsafe_code)]

use mcp_apps_server::{
    MetadataError, PermissionRequest, ResourceUi, ToolUi, UiCsp, UiPermissions, UiResourceUri,
    UiVisibility,
};
use rmcp::model::{MetaObject, Tool};
use serde_json::{Value, json};

fn meta(value: Value) -> MetaObject {
    MetaObject(value.as_object().expect("fixture is an object").clone())
}

#[test]
fn tool_association_preserves_schema_extensions_and_legacy_agreement()
-> Result<(), Box<dyn std::error::Error>> {
    let mut tool = Tool::new(
        "view",
        "Show a view",
        json!({"type": "object"}).as_object().unwrap().clone(),
    )
    .with_meta(meta(json!({
        "openai/example": {"enabled": true},
        "ui": {"visibility": ["app"], "future": {"value": 1}},
        "ui/resourceUri": "ui://example/view"
    })));
    let mut expected = tool.clone();
    expected.meta = Some(meta(json!({
        "openai/example": {"enabled": true},
        "ui": {"resourceUri": "ui://example/view", "visibility": ["app"], "future": {"value": 1}},
        "ui/resourceUri": "ui://example/view"
    })));
    let ui = ToolUi::new(UiResourceUri::new("ui://example/view")?);
    ui.apply_to(&mut tool)?;
    ui.apply_to(&mut tool)?;
    assert_eq!(tool, expected);
    assert_eq!(
        serde_json::to_value(ui.to_meta()?)?,
        json!({
            "ui": {"resourceUri": "ui://example/view"}
        })
    );
    assert_eq!(
        serde_json::to_value(ui.with_legacy_uri().to_meta()?)?,
        json!({
            "ui": {"resourceUri": "ui://example/view"},
            "ui/resourceUri": "ui://example/view"
        })
    );
    Ok(())
}

#[test]
fn tool_conflicts_are_atomic_and_report_the_protocol_field()
-> Result<(), Box<dyn std::error::Error>> {
    let mut ui = ToolUi::new(UiResourceUri::new("ui://example/view")?);
    ui.visibility = Some(vec![UiVisibility::App]);
    for (original, path) in [
        (
            json!({"ui/resourceUri": "ui://other/view", "other": true}),
            "_meta.ui/resourceUri",
        ),
        (json!({"ui/resourceUri": null}), "_meta.ui/resourceUri"),
        (
            json!({"ui": {"resourceUri": "ui://other/view"}}),
            "_meta.ui.resourceUri",
        ),
        (
            json!({"ui": {"visibility": ["model"]}}),
            "_meta.ui.visibility",
        ),
    ] {
        let mut tool = Tool::default().with_meta(meta(original));
        let before = tool.clone();
        assert!(
            matches!(ui.apply_to(&mut tool), Err(MetadataError::Conflict { path: actual }) if actual == path)
        );
        assert_eq!(tool, before);
    }
    for invalid in [Value::Null, json!(false), json!([]), json!("ui")] {
        let mut destination = meta(json!({"ui": invalid, "other": 1}));
        let before = destination.clone();
        assert!(matches!(
            ui.merge_into(&mut destination),
            Err(MetadataError::InvalidUi)
        ));
        assert_eq!(destination, before);
    }
    Ok(())
}

#[test]
fn resource_composition_preserves_unknown_nested_fields_and_rejects_policy_changes()
-> Result<(), Box<dyn std::error::Error>> {
    let mut csp = UiCsp::default();
    csp.connect_domains = Some(vec![]);
    let mut permissions = UiPermissions::default();
    permissions.camera = Some(PermissionRequest::new());
    let mut ui = ResourceUi::default();
    ui.csp = Some(csp);
    ui.permissions = Some(permissions);
    ui.prefers_border = Some(false);
    let mut destination = meta(json!({
        "other": {"keep": true},
        "ui": {"future": 1, "csp": {"futureDirective": ["custom"]},
            "permissions": {"camera": {"futureSetting": true}, "futurePermission": {}}}
    }));
    ui.merge_into(&mut destination)?;
    ui.merge_into(&mut destination)?;
    assert_eq!(
        serde_json::to_value(&destination)?,
        json!({
            "other": {"keep": true},
            "ui": {"future": 1, "prefersBorder": false,
                "csp": {"futureDirective": ["custom"], "connectDomains": []},
                "permissions": {"camera": {"futureSetting": true}, "futurePermission": {}}}
        })
    );
    ui.csp.as_mut().unwrap().resource_domains = Some(vec!["https://cdn.example".into()]);
    ui.prefers_border = Some(true);
    let before = destination.clone();
    assert!(
        matches!(ui.merge_into(&mut destination), Err(MetadataError::Conflict { path }) if path == "_meta.ui.prefersBorder")
    );
    assert_eq!(destination, before);
    let mut wrong_shape = meta(json!({"ui": {"permissions": {"camera": true}}}));
    let before = wrong_shape.clone();
    assert!(
        matches!(ui.merge_into(&mut wrong_shape), Err(MetadataError::Conflict { path }) if path == "_meta.ui.permissions.camera")
    );
    assert_eq!(wrong_shape, before);
    Ok(())
}

#[test]
fn empty_patches_preserve_absence_and_explicit_empty_visibility_is_retained()
-> Result<(), Box<dyn std::error::Error>> {
    let mut tool = Tool::default();
    ToolUi::default().apply_to(&mut tool)?;
    assert!(tool.meta.is_none());
    let mut destination = meta(json!({"ui": {"visibility": ["model", "app"]}}));
    let before = destination.clone();
    ToolUi::default().merge_into(&mut destination)?;
    ResourceUi::default().merge_into(&mut destination)?;
    assert_eq!(destination, before);
    let mut ui = ToolUi::default();
    ui.visibility = Some(vec![]);
    assert_eq!(
        serde_json::to_value(ui.to_meta()?)?,
        json!({"ui": {"visibility": []}})
    );
    assert!(matches!(
        ui.merge_into(&mut destination),
        Err(MetadataError::Conflict { .. })
    ));
    assert_eq!(destination, before);
    Ok(())
}
