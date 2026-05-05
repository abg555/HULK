use hulk::{
    CallExpr, Expr, FunctionDecl, Item, KindExpr, Program, SemanticAnalyzer, TypeRef,
    desugar_functors,
};

#[test]
fn lowers_function_type_annotation_to_synthetic_functor_protocol() {
    let lowered = desugar_functors(
        r#"
function test(filter: (Number) -> Boolean): Boolean => filter(3);
test((x: Number): Boolean => x > 0)
"#,
    )
    .expect("expected functor desugar to succeed");

    let functor_protocol = lowered.items.iter().find_map(|item| match item {
        Item::Protocol(proto) if proto.name.starts_with("_Functor") => Some(proto),
        _ => None,
    });
    assert!(
        functor_protocol.is_some(),
        "expected synthetic _Functor protocol"
    );

    let test = find_function(&lowered, "test").expect("expected test function");
    assert!(
        matches!(
            test.params.first().and_then(|param| param.types.as_ref()),
            Some(TypeRef::Custom(name)) if name.starts_with("_Functor")
        ),
        "expected function type annotation to become synthetic protocol"
    );
    assert_invokes_functor(&test.body);
}

#[test]
fn wraps_function_argument_as_functor_object() {
    let lowered = desugar_functors(
        r#"
protocol NumberFilter {
    invoke(x: Number): Boolean;
}

function is_odd(x: Number): Boolean => x % 2 == 1;
function test(filter: NumberFilter): Boolean => filter(3);
test(is_odd)
"#,
    )
    .expect("expected functor desugar to succeed");

    assert!(
        lowered.items.iter().any(|item| {
            matches!(item, Item::Type(typ) if typ.name.starts_with("_FunctorWrapper"))
        }),
        "expected synthetic wrapper type"
    );

    let global_call = lowered.items.iter().find_map(|item| match item {
        Item::GlobalExpr(expr) => Some(expr),
        _ => None,
    });
    let Some(KindExpr::Call(CallExpr { arguments, .. })) = global_call.map(|expr| &expr.kind)
    else {
        panic!("expected global function call");
    };
    assert!(
        matches!(
            arguments.first().map(|arg| &arg.kind),
            Some(KindExpr::New(new_expr)) if new_expr.type_name.starts_with("_FunctorWrapper")
        ),
        "expected function argument to be wrapped with new _FunctorWrapper"
    );
}

#[test]
fn lowered_functor_program_still_passes_semantic_analysis() {
    let lowered = desugar_functors(
        r#"
protocol NumberFilter {
    invoke(x: Number): Boolean;
}

type IsOdd {
    invoke(x: Number): Boolean => x % 2 == 1;
}

function is_odd(x: Number): Boolean => x % 2 == 1;
function test_protocol(filter: NumberFilter): Boolean => filter(3);
function test_function(filter: (Number) -> Boolean): Boolean => filter(3);
{
    test_protocol(is_odd);
    test_protocol((x: Number): Boolean => x > 0);
    test_protocol(IsOdd());
    test_function(new IsOdd());
}
"#,
    )
    .expect("expected functor desugar to succeed");

    let result = SemanticAnalyzer::new().analyze(&lowered);
    assert!(
        result.is_ok(),
        "expected lowered program to remain semantically valid, got: {result:?}"
    );
}

fn find_function<'a>(program: &'a Program, name: &str) -> Option<&'a FunctionDecl> {
    program.items.iter().find_map(|item| match item {
        Item::Function(func) if func.name == name => Some(func),
        _ => None,
    })
}

fn assert_invokes_functor(expr: &Expr) {
    let KindExpr::Call(call) = &expr.kind else {
        panic!("expected call expression");
    };
    assert!(
        matches!(
            &call.callee.kind,
            KindExpr::MemberAccess(member) if member.field == "invoke"
        ),
        "expected call callee to be lowered to .invoke"
    );
}
