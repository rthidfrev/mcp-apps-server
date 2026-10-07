//! Registration invariants and composition boundaries.

#![cfg(feature = "server")]
#![forbid(unsafe_code)]

use std::error::Error;

use mcp_apps_server::{
    AppRouter, MetadataError, RegistrationError, ResourceUi, StaticUiResource, ToolUi, UiResource,
    UiResourceRoute, UiResourceUri, UiUriError, UiVisibility,
};
use rmcp::{
    handler::server::{
        common::schema_for_empty_input,
        router::tool::{ToolRoute, ToolRouter},
    },
    model::{
        CallToolResult, MetaObject, PaginatedRequestParams, ServerCapabilities, Tool,
        ToolAnnotations,
    },
};
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn Error>>;

fn meta(value: Value) -> MetaObject {
    MetaObject(
        value
            .as_object()
            .expect("fixed fixture is an object")
            .clone(),
    )
}

fn tool(name: &str) -> Tool {
    Tool::new(
        name.to_owned(),
        "Registration fixture",
        schema_for_empty_input(),
    )
    .with_annotations(ToolAnnotations::new().read_only(true))
}

fn route(tool: Tool) -> ToolRoute<()> {
    ToolRoute::new_dyn(tool, |_| {
        Box::pin(async { Ok(CallToolResult::default().into()) })
    })
}

fn resource(uri: &str) -> Result<UiResource, UiUriError> {
    Ok(UiResource::new(UiResourceUri::new(uri)?, "fixture"))
}

struct StaticMetadata<const CASE: u8>;
impl<const CASE: u8> StaticUiResource for StaticMetadata<CASE> {
    fn resource() -> Result<UiResource, UiUriError> {
        resource("ui://metadata/view")
    }
    fn html() -> &'static str {
        "<!doctype html><html></html>"
    }
    fn meta() -> Result<MetaObject, MetadataError> {
        match CASE {
            1 => Ok(meta(json!({"ui": false}))),
            2 => Ok(meta(json!({"ui": {"prefersBorder": false}}))),
            3 => Err(MetadataError::Preparation(Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "extension annotation rejected",
            )))),
            _ => Ok(meta(json!({"example/display": {"compact": CASE == 0}}))),
        }
    }
    fn ui() -> ResourceUi {
        let mut ui = ResourceUi::default();
        ui.prefers_border = Some(true);
        ui
    }
}

fn prepare_static<const CASE: u8>(
    router: &mut AppRouter<()>,
    reuse: bool,
) -> Result<(), RegistrationError> {
    if reuse {
        router.ensure_static::<StaticMetadata<CASE>>().map(|_| ())
    } else {
        router.register_static::<StaticMetadata<CASE>>()
    }
}

#[test]
fn static_metadata_errors_preserve_causes_and_do_not_register_partial_resources() -> TestResult {
    // Exercise both public preparation paths: neither may skip validation or
    // swallow a consuming SDK's failure before inserting the resource.
    for reuse in [false, true] {
        let mut router = AppRouter::new();
        assert!(matches!(
            prepare_static::<1>(&mut router, reuse),
            Err(RegistrationError::Metadata(MetadataError::InvalidUi))
        ));
        assert!(matches!(prepare_static::<2>(&mut router, reuse),
            Err(RegistrationError::Metadata(MetadataError::Conflict { path }))
                if path == "_meta.ui.prefersBorder"));
        let error = prepare_static::<3>(&mut router, reuse).unwrap_err();
        let cause = error.source().unwrap().source().unwrap();
        assert_eq!(
            cause.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::InvalidData
        );
        assert!(router.list_resources(None)?.resources.is_empty());

        prepare_static::<0>(&mut router, reuse)?;
        let before = router.list_resources(None)?;
        assert!(prepare_static::<2>(&mut router, reuse).is_err());
        assert_eq!(router.list_resources(None)?, before);
    }
    Ok(())
}

#[test]
fn static_reuse_compares_additional_extension_metadata() -> TestResult {
    let mut router = AppRouter::<()>::new();
    router.register_static::<StaticMetadata<0>>()?;
    router.ensure_static::<StaticMetadata<0>>()?;
    // URI, descriptor, HTML and generic UI settings are identical; only the
    // consuming extension's preference differs, so reuse must fail.
    assert!(matches!(router.ensure_static::<StaticMetadata<4>>(),
        Err(RegistrationError::ConflictingResource { uri }) if uri == "ui://metadata/view"));
    assert_eq!(router.list_resources(None)?.resources.len(), 1);
    Ok(())
}

#[test]
fn association_preserves_rmcp_definitions_and_extension_metadata() -> TestResult {
    let original = tool("inventory").with_meta(meta(json!({
        "vendor/setting": {"enabled": true},
        "ui": {"future": {"value": 1}, "visibility": ["app"]},
        "ui/resourceUri": "ui://inventory/view"
    })));
    let mut router =
        AppRouter::from_tool_router(ToolRouter::new().with_route(route(original.clone())));
    router.register_resource(UiResourceRoute::static_html(
        resource("ui://inventory/view")?,
        "<!doctype html><html></html>",
    ))?;
    let mut ui = ToolUi::new(UiResourceUri::new("ui://inventory/view")?);
    ui.visibility = Some(vec![UiVisibility::App]);
    router.associate_tool("inventory", &ui)?;
    router.associate_tool("inventory", &ui)?;
    router.validate()?;

    let mut expected = original;
    expected.meta = Some(meta(json!({
        "vendor/setting": {"enabled": true},
        "ui": {
            "future": {"value": 1},
            "visibility": ["app"],
            "resourceUri": "ui://inventory/view"
        },
        "ui/resourceUri": "ui://inventory/view"
    })));
    assert_eq!(router.list_tools(None)?.tools, vec![expected]);
    Ok(())
}

#[test]
fn association_failures_leave_the_original_tool_unchanged() -> TestResult {
    let mut router = AppRouter::new();
    router.register_tool(route(tool("inventory")))?;
    let before = router.list_tools(None)?;
    let unregistered = ToolUi::new(UiResourceUri::new("ui://missing/view")?);
    assert!(matches!(
        router.associate_tool("unknown", &unregistered),
        Err(RegistrationError::UnknownTool { .. })
    ));
    assert!(matches!(
        router.associate_tool("inventory", &ToolUi::default()),
        Err(RegistrationError::MissingResourceUri { .. })
    ));
    assert!(matches!(
        router.associate_tool("inventory", &unregistered),
        Err(RegistrationError::UnregisteredResource { .. })
    ));
    assert_eq!(router.list_tools(None)?, before);

    router.register_resource(UiResourceRoute::static_html(
        resource("ui://inventory/view")?,
        "<!doctype html><html></html>",
    ))?;
    let mut ui = ToolUi::new(UiResourceUri::new("ui://inventory/view")?);
    ui.visibility = Some(vec![UiVisibility::Model]);
    router.associate_tool("inventory", &ui)?;
    let before_conflict = router.list_tools(None)?;
    ui.visibility = Some(vec![UiVisibility::App]);
    assert!(matches!(
        router.associate_tool("inventory", &ui),
        Err(RegistrationError::Metadata(MetadataError::Conflict { .. }))
    ));
    assert_eq!(router.list_tools(None)?, before_conflict);
    Ok(())
}

#[test]
fn validation_rejects_malformed_and_unresolved_imported_associations() -> TestResult {
    let cases = [
        (json!({"ui": false}), "metadata"),
        (json!({"ui": {"resourceUri": null}}), "type"),
        (json!({"ui/resourceUri": 7}), "type"),
        (json!({"ui": {"resourceUri": "https://example.com"}}), "uri"),
        (json!({"ui/resourceUri": "ui://bad uri"}), "uri"),
        (
            json!({"ui": {"resourceUri": "ui://missing/view"}}),
            "missing",
        ),
        (
            json!({
                "ui": {"resourceUri": "ui://inventory/view"},
                "ui/resourceUri": "ui://other/view"
            }),
            "metadata",
        ),
    ];
    for (declaration, expected) in cases {
        let original = tool("inventory").with_meta(meta(declaration));
        let mut imported = ToolRouter::new().with_route(route(original));
        imported.disable_route("inventory");
        let mut router = AppRouter::from_tool_router(imported);
        router.register_resource(UiResourceRoute::static_html(
            resource("ui://inventory/view")?,
            "<html></html>",
        ))?;
        let error = router
            .validate()
            .expect_err("disabled routes must also be validated");
        match expected {
            "metadata" => assert!(matches!(error, RegistrationError::Metadata(_))),
            "type" => assert!(matches!(
                error,
                RegistrationError::InvalidToolAssociation { .. }
            )),
            "uri" => assert!(matches!(error, RegistrationError::Uri(_))),
            "missing" => assert!(matches!(
                error,
                RegistrationError::UnregisteredResource { .. }
            )),
            _ => unreachable!("fixed case categories"),
        }
    }

    let original = tool("legacy").with_meta(meta(json!({
        "ui/resourceUri": "ui://inventory/view",
        "unknown": true
    })));
    let mut router =
        AppRouter::from_tool_router(ToolRouter::new().with_route(route(original.clone())));
    router.register_resource(UiResourceRoute::static_html(
        resource("ui://inventory/view")?,
        "<html></html>",
    ))?;
    router.validate()?;
    assert_eq!(router.list_tools(None)?.tools, vec![original]);
    Ok(())
}

#[test]
fn late_resource_collision_keeps_both_destination_registration_sets_unchanged() -> TestResult {
    let mut destination = AppRouter::new();
    destination.register_tool(route(tool("existing")))?;
    destination.register_resource(UiResourceRoute::static_html(
        resource("ui://shared/view")?,
        "original",
    ))?;
    let tools_before = destination.list_tools(None)?;
    let resources_before = destination.list_resources(None)?;

    let mut source = AppRouter::new();
    source.register_tool(route(tool("new-tool")))?;
    source.register_resource(UiResourceRoute::static_html(
        resource("ui://new/view")?,
        "new",
    ))?;
    source.register_resource(UiResourceRoute::static_html(
        resource("ui://shared/view")?,
        "replacement",
    ))?;
    assert!(matches!(
        destination.merge(source),
        Err(RegistrationError::DuplicateResource { uri }) if uri == "ui://shared/view"
    ));
    assert_eq!(destination.list_tools(None)?, tools_before);
    assert_eq!(destination.list_resources(None)?, resources_before);
    Ok(())
}

#[test]
fn disabled_tool_names_still_prevent_replacement_and_merge() -> TestResult {
    let original = tool("reserved");
    let mut imported = ToolRouter::new().with_route(route(original));
    imported.disable_route("reserved");
    let mut destination = AppRouter::from_tool_router(imported);
    destination.register_resource(UiResourceRoute::static_html(
        resource("ui://existing/view")?,
        "original",
    ))?;
    let resources_before = destination.list_resources(None)?;
    assert!(matches!(
        destination.register_tool(route(tool("reserved"))),
        Err(RegistrationError::DuplicateTool { name }) if name == "reserved"
    ));

    let mut source = AppRouter::new();
    source.register_tool(route(tool("reserved")))?;
    source.register_resource(UiResourceRoute::static_html(
        resource("ui://new/view")?,
        "new",
    ))?;
    assert!(matches!(
        destination.merge(source),
        Err(RegistrationError::DuplicateTool { name }) if name == "reserved"
    ));
    assert!(destination.list_tools(None)?.tools.is_empty());
    assert_eq!(destination.list_resources(None)?, resources_before);
    Ok(())
}

#[test]
fn merge_rejects_source_disable_markers_for_existing_destination_tools() -> TestResult {
    let mut destination = AppRouter::new();
    destination.register_tool(route(tool("existing")))?;
    let tools_before = destination.list_tools(None)?;
    let resources_before = destination.list_resources(None)?;

    let notifications = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let notified = notifications.clone();
    let mut imported = ToolRouter::new()
        .with_disabled("existing")
        .with_route(route(tool("incoming")));
    imported.set_notifier(move || {
        notified.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });
    let mut source = AppRouter::from_tool_router(imported);
    source.register_resource(UiResourceRoute::static_html(
        resource("ui://incoming/view")?,
        "incoming",
    ))?;

    assert!(matches!(destination.merge(source),
        Err(RegistrationError::DuplicateTool { name }) if name == "existing"));
    assert_eq!(destination.list_tools(None)?, tools_before);
    assert_eq!(destination.list_resources(None)?, resources_before);
    assert!(destination.get_tool("existing").is_some());
    assert_eq!(notifications.load(std::sync::atomic::Ordering::SeqCst), 0);
    Ok(())
}

#[test]
fn merge_preserves_disabled_source_routes_and_destination_policy() -> TestResult {
    let mut destination =
        AppRouter::from_tool_router(ToolRouter::new().with_disabled("destination-policy"));
    let source = AppRouter::from_tool_router(
        ToolRouter::new()
            .with_route(route(tool("source-policy")))
            .with_disabled("source-policy")
            .with_route(route(tool("destination-policy")))
            .with_route(route(tool("visible"))),
    );
    destination.merge(source)?;
    destination.validate()?;
    assert!(destination.get_tool("source-policy").is_none());
    assert!(destination.get_tool("destination-policy").is_none());
    assert!(destination.get_tool("visible").is_some());
    assert_eq!(destination.list_tools(None)?.tools.len(), 1);
    assert!(
        matches!(destination.register_tool(route(tool("source-policy"))),
        Err(RegistrationError::DuplicateTool { name }) if name == "source-policy")
    );
    Ok(())
}

#[test]
fn merge_rejects_divergent_source_names_without_changing_registration() -> TestResult {
    for declared_name in ["reserved", "incoming"] {
        let original = tool("reserved").with_meta(meta(json!({"vendor/original": true})));
        let mut destination = AppRouter::new();
        destination.register_tool(route(original))?;
        destination.register_resource(UiResourceRoute::static_html(
            resource("ui://existing/view")?,
            "original",
        ))?;
        let tools_before = destination.list_tools(None)?;
        let resources_before = destination.list_resources(None)?;

        let mut imported = ToolRouter::new();
        for lookup_name in ["first-alias", "second-alias"] {
            imported
                .map
                .insert(lookup_name.into(), route(tool(declared_name)));
        }
        let mut source = AppRouter::from_tool_router(imported);
        source.register_resource(UiResourceRoute::static_html(
            resource("ui://incoming/view")?,
            "incoming",
        ))?;

        assert!(matches!(
            destination.merge(source),
            Err(RegistrationError::ToolNameMismatch { declared_name: name, .. })
                if name == declared_name
        ));
        assert_eq!(destination.list_tools(None)?, tools_before);
        assert_eq!(destination.list_resources(None)?, resources_before);
        destination.validate()?;
    }
    Ok(())
}

#[test]
fn merge_rejects_divergent_destination_names_before_registration() -> TestResult {
    let mut imported = ToolRouter::new();
    imported
        .map
        .insert("lookup-name".into(), route(tool("declared-name")));
    let mut destination = AppRouter::from_tool_router(imported);
    let tools_before = destination.list_tools(None)?;
    let resources_before = destination.list_resources(None)?;
    let mut source = AppRouter::new();
    source.register_tool(route(tool("incoming")))?;
    source.register_resource(UiResourceRoute::static_html(
        resource("ui://incoming/view")?,
        "incoming",
    ))?;

    assert!(matches!(
        destination.merge(source),
        Err(RegistrationError::ToolNameMismatch { registered_name, declared_name })
            if registered_name == "lookup-name" && declared_name == "declared-name"
    ));
    assert_eq!(destination.list_tools(None)?, tools_before);
    assert_eq!(destination.list_resources(None)?, resources_before);
    Ok(())
}

#[test]
fn imported_cross_router_association_can_be_resolved_before_finalization() -> TestResult {
    let original = tool("inventory").with_meta(meta(json!({
        "ui": {"resourceUri": "ui://inventory/view"}
    })));
    let mut destination =
        AppRouter::from_tool_router(ToolRouter::new().with_route(route(original)));
    assert!(matches!(
        destination.validate(),
        Err(RegistrationError::UnregisteredResource { .. })
    ));
    let mut source = AppRouter::new();
    source.register_resource(UiResourceRoute::static_html(
        resource("ui://inventory/view")?,
        "<html></html>",
    ))?;
    destination.merge(source)?;
    destination.validate()?;
    Ok(())
}

#[test]
fn finalization_rejects_divergent_imported_tool_lookup_and_declaration_names() -> TestResult {
    let mut imported = ToolRouter::new();
    imported
        .map
        .insert("lookup-name".into(), route(tool("declared-name")));
    let router = AppRouter::from_tool_router(imported);
    let before = router.list_tools(None)?;
    assert!(matches!(
        router.validate(),
        Err(RegistrationError::ToolNameMismatch { registered_name, declared_name })
            if registered_name == "lookup-name" && declared_name == "declared-name"
    ));
    assert_eq!(router.list_tools(None)?, before);
    Ok(())
}

#[test]
fn registration_rejects_invalid_resource_metadata_and_foreign_pagination() -> TestResult {
    assert!(matches!(
        UiResourceRoute::<()>::static_html(resource("ui://view")?, "<html></html>")
            .with_meta(meta(json!({"ui": []}))),
        Err(MetadataError::InvalidUi)
    ));
    let mut router = AppRouter::new();
    router.register_tool(route(tool("fixture")))?;
    router.register_resource(UiResourceRoute::static_html(
        resource("ui://view")?,
        "<html></html>",
    ))?;
    let cursor = Some(PaginatedRequestParams::default().with_cursor(Some("foreign".into())));
    assert!(router.list_tools(cursor.clone()).is_err());
    assert!(router.list_resources(cursor).is_err());
    Ok(())
}

#[test]
fn capability_declaration_preserves_consumer_settings_and_extension_data() -> TestResult {
    let mut router = AppRouter::new();
    router.register_tool(route(tool("fixture")))?;
    router.register_resource(UiResourceRoute::static_html(
        resource("ui://view")?,
        "<html></html>",
    ))?;
    let mut capabilities: ServerCapabilities = serde_json::from_value(json!({
        "tools": {"listChanged": true},
        "resources": {"subscribe": true, "listChanged": false},
        "extensions": {
            "io.modelcontextprotocol/ui": {"future": 1},
            "vendor/extension": {"setting": true}
        }
    }))?;
    let before = capabilities.clone();
    router.declare_capabilities(&mut capabilities);
    assert_eq!(capabilities, before);
    Ok(())
}
