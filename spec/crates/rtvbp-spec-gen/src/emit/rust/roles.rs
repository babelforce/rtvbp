use std::collections::HashMap;
use std::fmt::Write as _;

use rtvbp_spec_model::Role;

use super::{RUST_BANNER, RustEmitError, event_constant, operation_constant, snake_wire_name};
use crate::resolve::{ResolvedCatalog, ResolvedEvent, ResolvedOperation};

#[derive(Clone, Copy)]
enum LocalRole {
    Application,
    Voice,
}

impl LocalRole {
    const ALL: [Self; 2] = [Self::Application, Self::Voice];

    const fn name(self) -> &'static str {
        match self {
            Self::Application => "Application",
            Self::Voice => "Voice",
        }
    }

    const fn lower(self) -> &'static str {
        match self {
            Self::Application => "application",
            Self::Voice => "voice",
        }
    }

    const fn spec_role(self) -> Role {
        match self {
            Self::Application => Role::Application,
            Self::Voice => Role::Voice,
        }
    }

    const fn owns(self, role: Role) -> bool {
        matches!(
            (self, role),
            (Self::Application, Role::Application) | (Self::Voice, Role::Voice) | (_, Role::Both)
        )
    }

    const fn receives(self, emitted_by: Role) -> bool {
        matches!(
            (self, emitted_by),
            (Self::Application, Role::Voice) | (Self::Voice, Role::Application) | (_, Role::Both)
        )
    }
}

pub(super) fn render(catalog: &ResolvedCatalog) -> Result<String, RustEmitError> {
    validate_method_names(catalog)?;
    let mut output = String::from(RUST_BANNER);
    output.push_str("use std::sync::Arc;\n\nuse async_trait::async_trait;\n\n");
    for role in LocalRole::ALL {
        render_handler_trait(&mut output, catalog, role);
        render_handler_adapters(&mut output, catalog, role);
    }
    for role in LocalRole::ALL {
        render_peer(&mut output, catalog, role);
    }
    for role in LocalRole::ALL {
        render_events(&mut output, catalog, role);
    }
    for role in LocalRole::ALL {
        render_event_handler_trait(&mut output, catalog, role);
        render_event_adapters(&mut output, catalog, role);
    }
    Ok(super::finish(output))
}

pub(super) fn render_tests(catalog: &ResolvedCatalog) -> String {
    let mut output = String::from(RUST_BANNER);
    output.push_str("use super::*;\n\n");
    output.push_str("#[test]\nfn generated_role_metadata_is_complete() {\n");
    for role in LocalRole::ALL {
        let owned = catalog
            .operations
            .iter()
            .filter(|operation| role.owns(operation.handled_by))
            .count()
            + catalog
                .operations
                .iter()
                .flat_map(|operation| &operation.rejections)
                .filter(|rejection| rejection.role == role.spec_role())
                .count();
        writeln!(
            output,
            "    assert_eq!({}_HANDLER_METHODS.len(), {owned});",
            role.lower().to_ascii_uppercase()
        )
        .unwrap();
    }
    output.push_str("}\n\n");
    for role in LocalRole::ALL {
        render_test_handler(&mut output, catalog, role);
        render_request_contract_test(&mut output, catalog, role);
        render_test_event_handler(&mut output, catalog, role);
        render_event_contract_test(&mut output, catalog, role);
    }
    super::finish(output)
}

fn render_test_handler(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    writeln!(
        output,
        "struct Generated{name}Handler;\n\n#[async_trait::async_trait]\nimpl {name}Handler for Generated{name}Handler {{"
    )
    .unwrap();
    for operation in catalog
        .operations
        .iter()
        .filter(|operation| role.owns(operation.handled_by))
    {
        let example = operation
            .examples
            .first()
            .expect("validated operations have an example");
        let request = serde_json::to_string(&example.request).unwrap();
        let response = serde_json::to_string(&example.response).unwrap();
        writeln!(
            output,
            "    async fn {}(&self, _context: crate::HandlerContext, request: {}) -> Result<{}, crate::Error> {{\n        let expected = serde_json::from_str::<serde_json::Value>({request:?}).unwrap();\n        assert_eq!(serde_json::to_value(request).unwrap(), expected);\n        Ok(serde_json::from_str({response:?}).unwrap())\n    }}",
            operation_name(operation),
            operation.request,
            operation.response,
        )
        .unwrap();
    }
    output.push_str("}\n\n");
}

fn render_request_contract_test(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    let lower = role.lower();
    let const_name = format!("{}_HANDLER_METHODS", lower.to_ascii_uppercase());
    writeln!(
        output,
        "#[tokio::test]\n#[allow(clippy::too_many_lines)]\nasync fn generated_{lower}_request_contract_is_executable() {{\n    let registrations = {lower}_handlers(Arc::new(Generated{name}Handler));\n    let methods = registrations.iter().map(crate::RequestRegistration::method).collect::<Vec<_>>();\n    assert_eq!(methods, {const_name});"
    )
    .unwrap();
    for operation in catalog
        .operations
        .iter()
        .filter(|operation| role.owns(operation.handled_by))
    {
        let example = operation
            .examples
            .first()
            .expect("validated operations have an example");
        let request = serde_json::to_string(&example.request).unwrap();
        let response = serde_json::to_string(&example.response).unwrap();
        let terminal_assertion = if operation.terminal {
            "assert!(reply.terminal);"
        } else {
            "assert!(!reply.terminal);"
        };
        writeln!(
            output,
            "    {{\n        let registration = registrations.iter().find(|registration| registration.method() == {}).unwrap();\n        let request = serde_json::from_str({request:?}).unwrap();\n        let reply = registration.handle(crate::HandlerContext::default(), request).await.unwrap();\n        assert_eq!(reply.payload, serde_json::from_str::<serde_json::Value>({response:?}).unwrap());\n        {terminal_assertion}\n    }}",
            operation_constant(&operation.method),
        )
        .unwrap();
    }
    for (operation, rejection) in catalog.operations.iter().flat_map(|operation| {
        operation
            .rejections
            .iter()
            .filter(move |rejection| rejection.role == role.spec_role())
            .map(move |rejection| (operation, rejection))
    }) {
        let example = operation
            .examples
            .first()
            .expect("validated operations have an example");
        let request = serde_json::to_string(&example.request).unwrap();
        writeln!(
            output,
            "    {{\n        let registration = registrations.iter().find(|registration| registration.method() == {}).unwrap();\n        let request = serde_json::from_str({request:?}).unwrap();\n        let error = registration.handle(crate::HandlerContext::default(), request).await.unwrap_err();\n        match error {{\n            crate::Error::Handler(error) => assert_eq!(error, crate::WireError {{ code: {}, message: {:?}.to_owned(), data: None }}),\n            other => panic!(\"unexpected rejection: {{other}}\"),\n        }}\n    }}",
            operation_constant(&operation.method),
            rejection.code,
            rejection.message,
        )
        .unwrap();
    }
    output.push_str("}\n\n");
}

fn render_test_event_handler(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    writeln!(
        output,
        "struct Generated{name}EventHandler;\n\n#[async_trait::async_trait]\nimpl {name}EventHandler for Generated{name}EventHandler {{"
    )
    .unwrap();
    for event in catalog
        .events
        .iter()
        .filter(|event| role.receives(event.emitted_by))
    {
        let example = event
            .examples
            .first()
            .expect("validated events have an example");
        let data = serde_json::to_string(&example.data).unwrap();
        writeln!(
            output,
            "    async fn {}(&self, _context: crate::HandlerContext, event: {}) -> Result<(), crate::Error> {{\n        let expected = serde_json::from_str::<serde_json::Value>({data:?}).unwrap();\n        assert_eq!(serde_json::to_value(event).unwrap(), expected);\n        Ok(())\n    }}",
            event_name(event),
            event.data,
        )
        .unwrap();
    }
    output.push_str("}\n\n");
}

fn render_event_contract_test(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    let lower = role.lower();
    let event_count = catalog
        .events
        .iter()
        .filter(|event| role.receives(event.emitted_by))
        .count();
    writeln!(
        output,
        "#[tokio::test]\nasync fn generated_{lower}_event_contract_is_executable() {{\n    let registrations = {lower}_event_handlers(Arc::new(Generated{name}EventHandler));\n    assert_eq!(registrations.len(), {event_count});"
    )
    .unwrap();
    for event in catalog
        .events
        .iter()
        .filter(|event| role.receives(event.emitted_by))
    {
        let example = event
            .examples
            .first()
            .expect("validated events have an example");
        let data = serde_json::to_string(&example.data).unwrap();
        writeln!(
            output,
            "    {{\n        let registration = registrations.iter().find(|registration| registration.event() == {}).unwrap();\n        let event = serde_json::from_str({data:?}).unwrap();\n        registration.handle(crate::HandlerContext::default(), event).await.unwrap();\n    }}",
            event_constant(&event.name),
        )
        .unwrap();
    }
    output.push_str("}\n\n");
}

fn render_handler_trait(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    writeln!(
        output,
        "/// Operations implemented by the local {name} role.\n#[async_trait]\npub trait {name}Handler: Send + Sync {{"
    )
    .unwrap();
    for operation in catalog
        .operations
        .iter()
        .filter(|operation| role.owns(operation.handled_by))
    {
        writeln!(
            output,
            "    async fn {}(\n        &self,\n        context: crate::HandlerContext,\n        request: {},\n    ) -> Result<{}, crate::Error>;",
            operation_name(operation),
            operation.request,
            operation.response
        )
        .unwrap();
    }
    output.push_str("}\n\n");
}

fn render_handler_adapters(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    let lower = role.lower();
    let const_name = format!("{}_HANDLER_METHODS", lower.to_ascii_uppercase());
    let owned = catalog
        .operations
        .iter()
        .filter(|operation| role.owns(operation.handled_by))
        .collect::<Vec<_>>();
    let rejections = catalog
        .operations
        .iter()
        .flat_map(|operation| {
            operation
                .rejections
                .iter()
                .filter(move |rejection| rejection.role == role.spec_role())
                .map(move |rejection| (operation, rejection))
        })
        .collect::<Vec<_>>();
    output.push_str("#[doc(hidden)]\n");
    writeln!(output, "pub const {const_name}: &[&str] = &[").unwrap();
    for operation in &owned {
        writeln!(output, "    {},", operation_constant(&operation.method)).unwrap();
    }
    for (operation, _) in &rejections {
        writeln!(output, "    {},", operation_constant(&operation.method)).unwrap();
    }
    output.push_str("];\n\n");
    writeln!(
        output,
        "/// Convert a role implementation into runtime request registrations.\npub fn {lower}_handlers(handler: Arc<dyn {name}Handler>) -> Vec<crate::RequestRegistration> {{"
    )
    .unwrap();
    if owned.is_empty() && rejections.is_empty() {
        output.push_str("    drop(handler);\n    Vec::new()\n}\n\n");
        return;
    }
    output.push_str("    let mut registrations = Vec::new();\n");
    for operation in owned {
        let method = operation_constant(&operation.method);
        let function = operation_name(operation);
        let request = &operation.request;
        let response = &operation.response;
        writeln!(
            output,
            "    {{\n        let handler = Arc::clone(&handler);\n        registrations.push(crate::RequestRegistration::typed::<{request}, {response}, _, _>(\n            {method},\n            {},\n            move |context, request| {{\n                let handler = Arc::clone(&handler);\n                async move {{ handler.{function}(context, request).await }}\n            }},\n        ));\n    }}",
            operation.terminal
        )
        .unwrap();
    }
    for (operation, rejection) in rejections {
        writeln!(
            output,
            "    registrations.push(crate::RequestRegistration::rejection(\n        {},\n        crate::WireError {{ code: {}, message: {:?}.to_owned(), data: None }},\n    ));",
            operation_constant(&operation.method),
            rejection.code,
            rejection.message
        )
        .unwrap();
    }
    output.push_str("    drop(handler);\n    registrations\n}\n\n");
}

fn render_peer(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    writeln!(
        output,
        "/// Typed client for operations implemented by a peer in the {name} role.\npub struct {name}Peer<R> {{\n    requester: R,\n}}\n\nimpl<R> {name}Peer<R> {{\n    #[must_use]\n    pub const fn new(requester: R) -> Self {{ Self {{ requester }} }}\n\n    /// Return the underlying raw requester.\n    pub fn into_inner(self) -> R {{ self.requester }}\n}}\n\nimpl<R: crate::Requester> {name}Peer<R> {{"
    )
    .unwrap();
    for operation in catalog
        .operations
        .iter()
        .filter(|operation| role.owns(operation.handled_by))
    {
        writeln!(
            output,
            "    /// Send the typed request through the underlying requester.\n    ///\n    /// # Errors\n    ///\n    /// Returns validation, transport, remote, or response-decoding failures.\n    pub async fn {}(&self, request: {}) -> Result<{}, crate::Error> {{\n        crate::request_peer(&self.requester, request).await\n    }}",
            operation_name(operation),
            operation.request,
            operation.response
        )
        .unwrap();
    }
    output.push_str("}\n\n");
}

fn render_events(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    writeln!(
        output,
        "/// Typed emitter for events owned by the {name} role.\npub struct {name}Events<N> {{\n    notifier: N,\n}}\n\nimpl<N> {name}Events<N> {{\n    #[must_use]\n    pub const fn new(notifier: N) -> Self {{ Self {{ notifier }} }}\n\n    /// Return the underlying raw notifier.\n    pub fn into_inner(self) -> N {{ self.notifier }}\n}}\n\nimpl<N: crate::Notifier> {name}Events<N> {{"
    )
    .unwrap();
    for event in catalog
        .events
        .iter()
        .filter(|event| role.owns(event.emitted_by))
    {
        writeln!(
            output,
            "    /// Emit the typed event through the underlying notifier.\n    ///\n    /// # Errors\n    ///\n    /// Returns validation, encoding, or transport failures.\n    pub async fn {}(&self, event: {}) -> Result<(), crate::Error> {{\n        crate::notify_event(&self.notifier, event).await\n    }}",
            event_name(event),
            event.data
        )
        .unwrap();
    }
    output.push_str("}\n\n");
}

fn render_event_handler_trait(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    writeln!(
        output,
        "/// Peer events received by the local {name} role.\n#[async_trait]\npub trait {name}EventHandler: Send + Sync {{"
    )
    .unwrap();
    for event in catalog
        .events
        .iter()
        .filter(|event| role.receives(event.emitted_by))
    {
        writeln!(
            output,
            "    async fn {}(\n        &self,\n        context: crate::HandlerContext,\n        event: {},\n    ) -> Result<(), crate::Error>;",
            event_name(event),
            event.data
        )
        .unwrap();
    }
    output.push_str("}\n\n");
}

fn render_event_adapters(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    let lower = role.lower();
    let events = catalog
        .events
        .iter()
        .filter(|event| role.receives(event.emitted_by))
        .collect::<Vec<_>>();
    writeln!(
        output,
        "/// Convert a role event implementation into runtime event registrations.\npub fn {lower}_event_handlers(handler: Arc<dyn {name}EventHandler>) -> Vec<crate::EventRegistration> {{"
    )
    .unwrap();
    if events.is_empty() {
        output.push_str("    drop(handler);\n    Vec::new()\n}\n\n");
        return;
    }
    output.push_str("    let mut registrations = Vec::new();\n");
    for event in events {
        let event_const = event_constant(&event.name);
        let function = event_name(event);
        let data = &event.data;
        writeln!(
            output,
            "    {{\n        let handler = Arc::clone(&handler);\n        registrations.push(crate::EventRegistration::typed::<{data}, _, _>(\n            {event_const},\n            move |context, event| {{\n                let handler = Arc::clone(&handler);\n                async move {{ handler.{function}(context, event).await }}\n            }},\n        ));\n    }}"
        )
        .unwrap();
    }
    output.push_str("    drop(handler);\n    registrations\n}\n\n");
}

fn validate_method_names(catalog: &ResolvedCatalog) -> Result<(), RustEmitError> {
    for role in LocalRole::ALL {
        let mut methods = HashMap::new();
        let handler_surface = format!("{}Handler", role.name());
        for operation in catalog
            .operations
            .iter()
            .filter(|operation| role.owns(operation.handled_by))
        {
            insert_method(
                &mut methods,
                &handler_surface,
                operation_name(operation),
                &operation.method,
            )?;
        }
        let event_surface = format!("{}EventHandler", role.name());
        methods.clear();
        for event in catalog
            .events
            .iter()
            .filter(|event| role.receives(event.emitted_by))
        {
            insert_method(&mut methods, &event_surface, event_name(event), &event.name)?;
        }
    }
    Ok(())
}

fn insert_method(
    methods: &mut HashMap<String, String>,
    surface: &str,
    method: String,
    wire_name: &str,
) -> Result<(), RustEmitError> {
    if let Some(first) = methods.insert(method.clone(), wire_name.to_owned()) {
        return Err(RustEmitError::RoleMethodCollision {
            surface: surface.to_owned(),
            method,
            first,
            second: wire_name.to_owned(),
        });
    }
    Ok(())
}

fn operation_name(operation: &ResolvedOperation) -> String {
    snake_wire_name(&operation.method)
}

fn event_name(event: &ResolvedEvent) -> String {
    snake_wire_name(&event.name)
}
