use std::collections::HashMap;
use std::fmt::Write as _;

use rtvbp_spec_model::Role;

use super::{GO_BANNER, GoEmitError, event_constant, operation_constant, pascal_wire_name};
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

pub(super) fn render(catalog: &ResolvedCatalog, package: &str) -> Result<String, GoEmitError> {
    validate_method_names(catalog)?;
    let mut output = String::from(GO_BANNER);
    writeln!(output, "package {package}\n").unwrap();
    output.push_str(
        "import (\n\t\"context\"\n\t\"encoding/json\"\n\t\"fmt\"\n\t\"reflect\"\n\n\t\"github.com/babelforce/rtvbp/sdk/go\"\n)\n\n",
    );
    output.push_str(
        "// Requester is the narrow request capability used by typed peer clients.\n\
         // Both *rtvbp.Session and rtvbp.SHC satisfy it.\n\
         type Requester interface {\n\
         \tRequest(context.Context, rtvbp.NamedRequest) (rtvbp.Response, error)\n\
         }\n\n\
         // Notifier is the narrow event capability used by role event emitters.\n\
         // rtvbp.SHC satisfies it.\n\
         type Notifier interface {\n\
         \tNotify(context.Context, rtvbp.NamedEvent) error\n\
         }\n\n",
    );

    for role in LocalRole::ALL {
        render_handler_interface(&mut output, catalog, role);
        render_handler_adapter(&mut output, catalog, role);
    }
    for role in LocalRole::ALL {
        render_peer(&mut output, catalog, role);
    }
    output.push_str(
        "func requestPeer[Result any](ctx context.Context, requester Requester, request rtvbp.NamedRequest) (*Result, error) {\n\
         \tif request == nil || reflect.ValueOf(request).Kind() == reflect.Pointer && reflect.ValueOf(request).IsNil() {\n\
         \t\treturn nil, fmt.Errorf(\"%w: request is nil\", rtvbp.ErrRequestValidationFailed)\n\
         \t}\n\
         \tif validation, ok := request.(rtvbp.Validation); ok {\n\
         \t\tif err := validation.Validate(); err != nil {\n\
         \t\t\treturn nil, fmt.Errorf(\"%w: request for %s: %w\", rtvbp.ErrRequestValidationFailed, request.MethodName(), err)\n\
         \t\t}\n\
         \t}\n\
         \tresponse, err := requester.Request(ctx, request)\n\
         \tif err != nil {\n\
         \t\treturn nil, err\n\
         \t}\n\
         \tpayload := response.Payload\n\
         \tif len(payload) == 0 {\n\
         \t\tpayload = json.RawMessage(\"{}\")\n\
         \t}\n\
         \tresult := new(Result)\n\
         \tif err := json.Unmarshal(payload, result); err != nil {\n\
         \t\treturn nil, fmt.Errorf(\"decode response for %s: %w\", request.MethodName(), err)\n\
         \t}\n\
         \tif validation, ok := any(result).(rtvbp.Validation); ok {\n\
         \t\tif err := validation.Validate(); err != nil {\n\
         \t\t\treturn nil, fmt.Errorf(\"validate response for %s: %w\", request.MethodName(), err)\n\
         \t\t}\n\
         \t}\n\
         \treturn result, nil\n\
         }\n\n\
         func notifyEvent(ctx context.Context, notifier Notifier, event rtvbp.NamedEvent) error {\n\
         \tif event == nil || reflect.ValueOf(event).Kind() == reflect.Pointer && reflect.ValueOf(event).IsNil() {\n\
         \t\treturn fmt.Errorf(\"%w: event is nil\", rtvbp.ErrRequestValidationFailed)\n\
         \t}\n\
         \tif validation, ok := event.(rtvbp.Validation); ok {\n\
         \t\tif err := validation.Validate(); err != nil {\n\
         \t\t\treturn fmt.Errorf(\"%w: event %s: %w\", rtvbp.ErrRequestValidationFailed, event.EventName(), err)\n\
         \t\t}\n\
         \t}\n\
         \treturn notifier.Notify(ctx, event)\n\
         }\n\n",
    );
    for role in LocalRole::ALL {
        render_events(&mut output, catalog, role);
    }
    for role in LocalRole::ALL {
        render_event_handler_interface(&mut output, catalog, role);
        render_event_handler_adapter(&mut output, catalog, role);
    }
    while output.ends_with('\n') {
        output.pop();
    }
    output.push('\n');
    Ok(output)
}

pub(super) fn render_tests(catalog: &ResolvedCatalog, package: &str) -> String {
    let mut output = String::from(GO_BANNER);
    writeln!(output, "package {package}\n").unwrap();
    output.push_str(
        "import (\n\t\"context\"\n\t\"encoding/json\"\n\t\"errors\"\n\t\"testing\"\n\n\t\"github.com/babelforce/rtvbp/sdk/go\"\n)\n\n",
    );
    render_test_fixtures(&mut output, catalog);
    for role in LocalRole::ALL {
        render_test_handler(&mut output, catalog, role);
        render_test_event_handler(&mut output, catalog, role);
    }
    render_test_requester(&mut output);
    render_adapter_test(&mut output);
    render_peer_test(&mut output, catalog);
    render_event_test(&mut output, catalog);
    render_unknown_test(&mut output);
    while output.ends_with('\n') {
        output.pop();
    }
    output.push('\n');
    output
}

fn render_test_fixtures(output: &mut String, catalog: &ResolvedCatalog) {
    let operation_width = catalog
        .operations
        .iter()
        .map(|operation| operation_constant_name(operation).len() + 1)
        .max()
        .unwrap_or(0);
    let event_width = catalog
        .events
        .iter()
        .map(|event| event_constant_name(event).len() + 1)
        .max()
        .unwrap_or(0);
    output.push_str("var roleTestRequests = map[string]json.RawMessage{\n");
    for operation in &catalog.operations {
        let request = serde_json::to_string(&operation.examples[0].request).unwrap();
        let key = format!("{}:", operation_constant_name(operation));
        writeln!(
            output,
            "\t{key:<operation_width$} json.RawMessage({request:?}),"
        )
        .unwrap();
    }
    output.push_str("}\n\nvar roleTestResponses = map[string]json.RawMessage{\n");
    for operation in &catalog.operations {
        let response = serde_json::to_string(&operation.examples[0].response).unwrap();
        let key = format!("{}:", operation_constant_name(operation));
        writeln!(
            output,
            "\t{key:<operation_width$} json.RawMessage({response:?}),"
        )
        .unwrap();
    }
    output.push_str("}\n\nvar roleTestEvents = map[string]json.RawMessage{\n");
    for event in &catalog.events {
        let data = serde_json::to_string(&event.examples[0].data).unwrap();
        let key = format!("{}:", event_constant_name(event));
        writeln!(output, "\t{key:<event_width$} json.RawMessage({data:?}),").unwrap();
    }
    output.push_str("}\n\nvar roleTestTerminal = map[string]bool{\n");
    for operation in &catalog.operations {
        let key = format!("{}:", operation_constant_name(operation));
        writeln!(output, "\t{key:<operation_width$} {},", operation.terminal,).unwrap();
    }
    output.push_str("}\n\nvar roleTestRejections = map[string]rtvbp.WireError{\n");
    for role in LocalRole::ALL {
        let lower = role.name().to_ascii_lowercase();
        for operation in &catalog.operations {
            for rejection in operation
                .rejections
                .iter()
                .filter(|rejection| rejection.role == role.spec_role())
            {
                writeln!(
                    output,
                    "\t{:?}: {{Code: {}, Message: {:?}}},",
                    format!("{lower}/{}", operation.method),
                    rejection.code,
                    rejection.message,
                )
                .unwrap();
            }
        }
    }
    output.push_str(
        "}\n\nfunc roleTestValue[T any](payload json.RawMessage) *T {\n\tvalue := new(T)\n\tif err := json.Unmarshal(payload, value); err != nil {\n\t\tpanic(err)\n\t}\n\treturn value\n}\n\n",
    );
}

fn render_test_handler(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let lower = role.name().to_ascii_lowercase();
    writeln!(output, "type {lower}RoleTestHandler struct{{}}\n").unwrap();
    for operation in catalog
        .operations
        .iter()
        .filter(|operation| role.owns(operation.handled_by))
    {
        writeln!(
            output,
            "func (*{lower}RoleTestHandler) {}(context.Context, rtvbp.SHC, *{}) (*{}, error) {{\n\treturn roleTestValue[{}](roleTestResponses[{}]), nil\n}}\n",
            operation_name(operation),
            operation.request,
            operation.response,
            operation.response,
            operation_constant_name(operation)
        )
        .unwrap();
    }
}

fn render_test_event_handler(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let lower = role.name().to_ascii_lowercase();
    writeln!(output, "type {lower}RoleTestEventHandler struct{{}}\n").unwrap();
    for event in catalog
        .events
        .iter()
        .filter(|event| role.receives(event.emitted_by))
    {
        writeln!(
            output,
            "func (*{lower}RoleTestEventHandler) {}(context.Context, rtvbp.SHC, *{}) error {{\n\treturn nil\n}}\n",
            event_name(event),
            event.data
        )
        .unwrap();
    }
}

fn render_test_requester(output: &mut String) {
    output.push_str(
        "type roleTestRequest struct{}\n\nfunc (*roleTestRequest) MethodName() string { return \"role.test\" }\n\ntype roleTestInvalidRequest struct{}\n\nfunc (*roleTestInvalidRequest) MethodName() string { return \"role.invalid\" }\nfunc (*roleTestInvalidRequest) Validate() error    { return errors.New(\"invalid request\") }\n\ntype roleTestInvalidEvent struct{}\n\nfunc (*roleTestInvalidEvent) EventName() string { return \"role.invalid\" }\nfunc (*roleTestInvalidEvent) Validate() error   { return errors.New(\"invalid event\") }\n\ntype roleTestRequester struct {\n\tempty bool\n\tcalls int\n}\n\nfunc (requester *roleTestRequester) Request(_ context.Context, request rtvbp.NamedRequest) (rtvbp.Response, error) {\n\trequester.calls++\n\tif requester.empty {\n\t\treturn rtvbp.Response{}, nil\n\t}\n\treturn rtvbp.Response{Payload: roleTestResponses[request.MethodName()]}, nil\n}\n\ntype roleTestNotifier struct {\n\tnames []string\n}\n\nfunc (notifier *roleTestNotifier) Notify(_ context.Context, event rtvbp.NamedEvent) error {\n\tnotifier.names = append(notifier.names, event.EventName())\n\treturn nil\n}\n\n",
    );
}

fn render_adapter_test(output: &mut String) {
    output.push_str(
        "func TestGeneratedRoleAdapters(t *testing.T) {\n\ttests := []struct {\n\t\tname string\n\t\trequests []any\n\t\tevents []any\n\t}{\n\t\t{name: \"application\", requests: ApplicationHandlers(&applicationRoleTestHandler{}), events: ApplicationEventHandlers(&applicationRoleTestEventHandler{})},\n\t\t{name: \"voice\", requests: VoiceHandlers(&voiceRoleTestHandler{}), events: VoiceEventHandlers(&voiceRoleTestEventHandler{})},\n\t}\n\tfor _, tc := range tests {\n\t\tt.Run(tc.name+\"/requests\", func(t *testing.T) {\n\t\t\tfor _, registration := range tc.requests {\n\t\t\t\thandler := registration.(rtvbp.RequestHandler)\n\t\t\t\tshc := rtvbp.NewTestingSHC()\n\t\t\t\terr := handler.Handle(context.Background(), shc, rtvbp.Request{Method: handler.MethodName(), Payload: roleTestRequests[handler.MethodName()]})\n\t\t\t\tif rejection, ok := roleTestRejections[tc.name+\"/\"+handler.MethodName()]; ok {\n\t\t\t\t\tvar handlerError *rtvbp.HandlerError\n\t\t\t\t\tif !errors.As(err, &handlerError) || handlerError.WireError.Code != rejection.Code || handlerError.WireError.Message != rejection.Message {\n\t\t\t\t\t\tt.Fatalf(\"%s rejection = %#v, want %#v\", handler.MethodName(), err, rejection)\n\t\t\t\t\t}\n\t\t\t\t\tif got := shc.State(); got != rtvbp.SessionStateActive {\n\t\t\t\t\t\tt.Fatalf(\"%s rejection state = %s, want active\", handler.MethodName(), got)\n\t\t\t\t\t}\n\t\t\t\t\tcontinue\n\t\t\t\t}\n\t\t\t\tif err != nil {\n\t\t\t\t\tt.Fatalf(\"%s: %v\", handler.MethodName(), err)\n\t\t\t\t}\n\t\t\t\twant := rtvbp.SessionStateActive\n\t\t\t\tif roleTestTerminal[handler.MethodName()] {\n\t\t\t\t\twant = rtvbp.SessionStateClosed\n\t\t\t\t}\n\t\t\t\tif got := shc.State(); got != want {\n\t\t\t\t\tt.Fatalf(\"%s state = %s, want %s\", handler.MethodName(), got, want)\n\t\t\t\t}\n\t\t\t}\n\t\t})\n\t\tt.Run(tc.name+\"/events\", func(t *testing.T) {\n\t\t\tfor _, registration := range tc.events {\n\t\t\t\thandler := registration.(rtvbp.EventHandler)\n\t\t\t\terr := handler.Handle(context.Background(), rtvbp.NewTestingSHC(), rtvbp.Event{Name: handler.EventName(), Payload: roleTestEvents[handler.EventName()]})\n\t\t\t\tif err != nil {\n\t\t\t\t\tt.Fatalf(\"%s: %v\", handler.EventName(), err)\n\t\t\t\t}\n\t\t\t}\n\t\t})\n\t}\n}\n\n",
    );
}

fn render_peer_test(output: &mut String, catalog: &ResolvedCatalog) {
    output
        .push_str("func TestGeneratedTypedPeers(t *testing.T) {\n\tctx := context.Background()\n");
    for role in LocalRole::ALL {
        let variable = role.name().to_ascii_lowercase();
        let name = role.name();
        writeln!(
            output,
            "\t{variable} := New{name}Peer(&roleTestRequester{{}})"
        )
        .unwrap();
        for operation in catalog
            .operations
            .iter()
            .filter(|operation| role.owns(operation.handled_by))
        {
            writeln!(
                output,
                "\tif _, err := {variable}.{}(ctx, roleTestValue[{}](roleTestRequests[{}])); err != nil {{\n\t\tt.Fatalf({:?}, err)\n\t}}",
                operation_name(operation),
                operation.request,
                operation_constant_name(operation),
                format!("{}.{}: %v", role.name(), operation_name(operation))
            )
            .unwrap();
        }
    }
    writeln!(
        output,
        "\tempty, err := requestPeer[map[string]any](ctx, &roleTestRequester{{empty: true}}, &roleTestRequest{{}})\n\tif err != nil || empty == nil || len(*empty) != 0 {{\n\t\tt.Fatalf(\"empty response = %#v, %v\", empty, err)\n\t}}\n\tboundary := &roleTestRequester{{}}\n\tif _, err := requestPeer[map[string]any](ctx, boundary, &roleTestInvalidRequest{{}}); !errors.Is(err, rtvbp.ErrRequestValidationFailed) || boundary.calls != 0 {{\n\t\tt.Fatalf(\"invalid request error = %v, calls = %d\", err, boundary.calls)\n\t}}\n}}\n"
    )
    .unwrap();
}

fn render_event_test(output: &mut String, catalog: &ResolvedCatalog) {
    output.push_str(
        "func TestGeneratedRoleEventEmitters(t *testing.T) {\n\tctx := context.Background()\n",
    );
    let mut expected = 0;
    for role in LocalRole::ALL {
        let variable = role.name().to_ascii_lowercase();
        let name = role.name();
        writeln!(
            output,
            "\t{variable}Notifier := &roleTestNotifier{{}}\n\t{variable} := New{name}Events({variable}Notifier)"
        )
        .unwrap();
        for event in catalog
            .events
            .iter()
            .filter(|event| role.owns(event.emitted_by))
        {
            expected += 1;
            writeln!(
                output,
                "\tif err := {variable}.{}(ctx, roleTestValue[{}](roleTestEvents[{}])); err != nil {{\n\t\tt.Fatal(err)\n\t}}",
                event_name(event),
                event.data,
                event_constant_name(event)
            )
            .unwrap();
        }
    }
    writeln!(
        output,
        "\tif got := len(applicationNotifier.names) + len(voiceNotifier.names); got != {expected} {{\n\t\tt.Fatalf(\"emitted event count = %d, want {expected}\", got)\n\t}}\n\tboundary := &roleTestNotifier{{}}\n\tif err := notifyEvent(ctx, boundary, &roleTestInvalidEvent{{}}); !errors.Is(err, rtvbp.ErrRequestValidationFailed) || len(boundary.names) != 0 {{\n\t\tt.Fatalf(\"invalid event error = %v, notifications = %v\", err, boundary.names)\n\t}}\n}}\n"
    )
    .unwrap();
}

fn render_unknown_test(output: &mut String) {
    output.push_str(
        "func TestGeneratedAdaptersPreserveUnknownHooks(t *testing.T) {\n\tregistrations := ApplicationHandlers(&applicationRoleTestHandler{})\n\tregistrations = append(registrations, ApplicationEventHandlers(&applicationRoleTestEventHandler{})...)\n\tdefaultHandler := rtvbp.NewHandler(rtvbp.HandlerConfig{}, registrations...)\n\terr := defaultHandler.OnRequest(context.Background(), rtvbp.NewTestingSHC(), rtvbp.Request{Method: \"unknown.method\"})\n\tvar handlerError *rtvbp.HandlerError\n\tif !errors.As(err, &handlerError) || handlerError.WireError.Code != 501 {\n\t\tt.Fatalf(\"unknown method error = %#v\", err)\n\t}\n\tif err := defaultHandler.OnEvent(context.Background(), rtvbp.NewTestingSHC(), rtvbp.Event{Name: \"unknown.event\"}); err != nil {\n\t\tt.Fatalf(\"unknown event: %v\", err)\n\t}\n\n\tmethodHooked, eventHooked := false, false\n\thooked := rtvbp.NewHandler(rtvbp.HandlerConfig{\n\t\tOnUnknownMethod: func(context.Context, rtvbp.SHC, rtvbp.Request) error { methodHooked = true; return rtvbp.NotImplemented(\"hooked\") },\n\t\tOnUnknownEvent: func(context.Context, rtvbp.SHC, rtvbp.Event) error { eventHooked = true; return nil },\n\t}, registrations...)\n\t_ = hooked.OnRequest(context.Background(), rtvbp.NewTestingSHC(), rtvbp.Request{Method: \"unknown.method\"})\n\t_ = hooked.OnEvent(context.Background(), rtvbp.NewTestingSHC(), rtvbp.Event{Name: \"unknown.event\"})\n\tif !methodHooked || !eventHooked {\n\t\tt.Fatalf(\"hooks = method:%v event:%v\", methodHooked, eventHooked)\n\t}\n}\n\n",
    );
}

fn render_handler_interface(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    writeln!(
        output,
        "// {name}Handler implements operations handled by the {name} role.\ntype {name}Handler interface {{"
    )
    .unwrap();
    for operation in catalog
        .operations
        .iter()
        .filter(|operation| role.owns(operation.handled_by))
    {
        writeln!(
            output,
            "\t{}(context.Context, rtvbp.SHC, *{}) (*{}, error)",
            operation_name(operation),
            operation.request,
            operation.response
        )
        .unwrap();
    }
    output.push_str("}\n\n");
}

fn render_handler_adapter(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    writeln!(
        output,
        "// {name}Handlers adapts a {name}Handler for rtvbp.NewHandler.\nfunc {name}Handlers(handler {name}Handler) []any {{\n\treturn []any{{"
    )
    .unwrap();
    for operation in &catalog.operations {
        if role.owns(operation.handled_by) {
            let adapter = if operation.terminal {
                "HandleTerminalRequest"
            } else {
                "HandleRequest"
            };
            writeln!(
                output,
                "\t\trtvbp.{adapter}(handler.{}),",
                operation_name(operation)
            )
            .unwrap();
        } else if let Some(rejection) = operation
            .rejections
            .iter()
            .find(|rejection| rejection.role == role.spec_role())
        {
            writeln!(
                output,
                "\t\trtvbp.HandleWithError[*{}](rtvbp.WireError{{Code: {}, Message: {:?}}}),",
                operation.request, rejection.code, rejection.message,
            )
            .unwrap();
        }
    }
    output.push_str("\t}\n}\n\n");
}

fn render_peer(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    writeln!(
        output,
        "// {name}Peer is a typed client for operations offered by the {name} role.\ntype {name}Peer struct {{\n\trequester Requester\n}}\n\n// New{name}Peer creates a typed {name} peer client.\nfunc New{name}Peer(requester Requester) *{name}Peer {{\n\treturn &{name}Peer{{requester: requester}}\n}}\n"
    )
    .unwrap();
    for operation in catalog
        .operations
        .iter()
        .filter(|operation| role.owns(operation.handled_by))
    {
        let method = operation_name(operation);
        writeln!(
            output,
            "// {method} calls {}.\nfunc (peer *{name}Peer) {method}(ctx context.Context, request *{}) (*{}, error) {{\n\treturn requestPeer[{}](ctx, peer.requester, request)\n}}\n",
            operation.method,
            operation.request,
            operation.response,
            operation.response
        )
        .unwrap();
    }
}

fn render_events(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    writeln!(
        output,
        "// {name}Events emits events owned by the {name} role.\ntype {name}Events struct {{\n\tnotifier Notifier\n}}\n\n// New{name}Events creates a typed {name} event emitter.\nfunc New{name}Events(notifier Notifier) *{name}Events {{\n\treturn &{name}Events{{notifier: notifier}}\n}}\n"
    )
    .unwrap();
    for event in catalog
        .events
        .iter()
        .filter(|event| role.owns(event.emitted_by))
    {
        let method = event_name(event);
        writeln!(
            output,
            "// {method} emits {}.\nfunc (events *{name}Events) {method}(ctx context.Context, event *{}) error {{\n\treturn notifyEvent(ctx, events.notifier, event)\n}}\n",
            event.name, event.data
        )
        .unwrap();
    }
}

fn render_event_handler_interface(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    writeln!(
        output,
        "// {name}EventHandler receives events emitted by the peer role.\ntype {name}EventHandler interface {{"
    )
    .unwrap();
    for event in catalog
        .events
        .iter()
        .filter(|event| role.receives(event.emitted_by))
    {
        writeln!(
            output,
            "\t{}(context.Context, rtvbp.SHC, *{}) error",
            event_name(event),
            event.data
        )
        .unwrap();
    }
    output.push_str("}\n\n");
}

fn render_event_handler_adapter(output: &mut String, catalog: &ResolvedCatalog, role: LocalRole) {
    let name = role.name();
    writeln!(
        output,
        "// {name}EventHandlers adapts a {name}EventHandler for rtvbp.NewHandler.\nfunc {name}EventHandlers(handler {name}EventHandler) []any {{\n\treturn []any{{"
    )
    .unwrap();
    for event in catalog
        .events
        .iter()
        .filter(|event| role.receives(event.emitted_by))
    {
        writeln!(
            output,
            "\t\trtvbp.HandleEvent(handler.{}),",
            event_name(event)
        )
        .unwrap();
    }
    output.push_str("\t}\n}\n\n");
}

fn operation_name(operation: &ResolvedOperation) -> String {
    pascal_wire_name(&operation.method)
}

fn event_name(event: &ResolvedEvent) -> String {
    pascal_wire_name(&event.name)
}

fn operation_constant_name(operation: &ResolvedOperation) -> String {
    operation_constant(&operation.method)
}

fn event_constant_name(event: &ResolvedEvent) -> String {
    event_constant(&event.name)
}

fn validate_method_names(catalog: &ResolvedCatalog) -> Result<(), GoEmitError> {
    for role in LocalRole::ALL {
        let name = role.name();
        ensure_unique_methods(
            format!("{name}Handler"),
            catalog
                .operations
                .iter()
                .filter(|operation| role.owns(operation.handled_by))
                .map(|operation| (operation.method.as_str(), operation_name(operation))),
        )?;
        ensure_unique_methods(
            format!("{name}Events"),
            catalog
                .events
                .iter()
                .filter(|event| role.owns(event.emitted_by))
                .map(|event| (event.name.as_str(), event_name(event))),
        )?;
        ensure_unique_methods(
            format!("{name}EventHandler"),
            catalog
                .events
                .iter()
                .filter(|event| role.receives(event.emitted_by))
                .map(|event| (event.name.as_str(), event_name(event))),
        )?;
    }
    Ok(())
}

fn ensure_unique_methods<'a>(
    surface: String,
    items: impl Iterator<Item = (&'a str, String)>,
) -> Result<(), GoEmitError> {
    let mut methods = HashMap::new();
    for (wire_name, method) in items {
        if let Some(first) = methods.insert(method.clone(), wire_name) {
            return Err(GoEmitError::RoleMethodCollision {
                surface,
                method,
                first: first.to_owned(),
                second: wire_name.to_owned(),
            });
        }
    }
    Ok(())
}
