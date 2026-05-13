#![allow(dead_code)]

#[derive(Clone)]
pub struct Program {
    pub items: Vec<Item>, //nodo principal del arbol
}

#[derive(Clone)]
pub enum Item {
    Import(ImportDecl),
    Export(ExportDecl),
    Function(FunctionDecl),
    Type(TypeDecl),
    Protocol(ProtocolDecl),
    Macro(MacroDecl),
    GlobalExpr(Expr),
}

#[derive(Clone)]
pub struct ImportDecl {
    pub module: String,
}

#[derive(Clone)]
pub struct ExportDecl {
    pub module: String,
}

#[derive(Clone)]
pub struct FunctionDecl {
    pub name: String, //ej: function sum(x, y) => x + y;
    pub params: Vec<Param>,
    pub return_type: Option<TypeRef>,
    pub body: Expr,
}

#[derive(Clone)]
pub struct Param {
    pub name: String,
    pub types: Option<TypeRef>,
    pub is_variadic: bool,
}

#[derive(Clone)]
pub struct TypeDecl {
    pub name: String, //ej: type Person{ name:String   mynameis()=> print(self.name)}
    pub param: Vec<Param>, //ej: type Point(x,y)...
    pub parent: Option<TypeRef>, //herencia
    pub parent_arg: Vec<Expr>, //parametros que se le pasan al padre
    pub fields: Vec<FieldDecl>,
    pub methods: Vec<FunctionDecl>,
}

#[derive(Clone)]
pub struct FieldDecl {
    pub name: String,
    pub type_annotation: Option<TypeRef>,
    pub initializer: Expr,
}

#[derive(Clone)]
pub struct ProtocolDecl {
    pub name: String,                 //ej: protocol Printable {print(): String}
    pub parent: Option<Box<TypeRef>>, // ej: protocol Equatable extends Hashable
    pub methods: Vec<ProtocolMethod>,
}

#[derive(Clone)]
pub struct ProtocolMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeRef,
}

#[derive(Clone)]
pub struct MacroDecl {
    pub name: String,
    pub params: Vec<MacroParam>,
    pub body: Expr,
}

#[derive(Clone)]
pub struct MacroParam {
    pub name: String,
    pub kind: MacroParamKind,
    pub type_info: Option<TypeRef>, // Type annotation for macro parameters
}

#[derive(Clone)]
pub enum MacroParamKind {
    Normal,
    Block,       // *expr
    Symbolic,    // @x
    Placeholder, // $x
}

#[derive(Clone)]
pub struct Expr {
    pub id: NodeId,     //id de la expresion
    pub span: Span, //para saber en que posicion se encuentra, util para debugueo y mostrar errores
    pub kind: KindExpr, //tipo de la expresion
}

pub fn mk_expr(kind: KindExpr) -> Expr {
    Expr {
        id: NodeId(0),
        span: Span { start: 0, end: 0 },
        kind,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32); //el campo id es de tipo NodeId que realmente es u32

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    pub start: usize, //posiciones dentro de un string, y los strings en Rust se indexan con usize
    pub end: usize,
}

#[derive(Clone)]
pub enum KindExpr {
    Literal(LiteralExpr),
    Variable(VariableExpr),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    Call(CallExpr),
    BaseCall(BaseCallExpr),
    MacroCall(MacroCallExpr),
    Let(LetExpr),
    Block(BlockExpr),
    If(IfExpr),
    While(WhileExpr),
    For(ForExpr),
    Assign(AssignExpr),
    MemberAccess(MemberAccessExpr),
    Index(IndexExpr),
    Array(ArrayExpr),
    ArrayComprehension(ArrayComprehensionExpr),
    Lambda(LambdaExpr),
    New(NewExpr),
    Is(IsExpr),
    As(AsExpr),
    Match(MatchExpr),
}

#[derive(Clone)]
pub struct LiteralExpr {
    pub value: LiteralValue, //ej: 5, "hola mundo", true
}

#[derive(Clone)]
pub enum LiteralValue {
    String(String),
    Number(f64),
    Bool(bool),
}

#[derive(Clone)]
pub struct VariableExpr {
    pub name: String, //ej: x
}

#[derive(Clone)]
pub struct BinaryExpr {
    pub left: Box<Expr>, //ej: 5+3
    pub operator: BinaryOperator,
    pub right: Box<Expr>,
}

#[derive(Clone)]
pub enum BinaryOperator {
    //aritmeticos
    Add,
    Sub,
    Mul,
    Div,
    Pow,
    Mod,
    //comparacion
    Equal,
    NotEqual,
    Less,
    Greater,
    LessEqual,
    GreaterEqual,
    //logicos
    And,
    Or,
    // Cadenas
    Concat,     // @
    FullConcat, // @@ (concatena con espacio)
}

#[derive(Clone)]
pub struct UnaryExpr {
    pub operator: UnaryOperator, //ej: !true, -5
    pub right: Box<Expr>,
}

#[derive(Clone)]
pub enum UnaryOperator {
    Negate,
    Not,
}

#[derive(Clone)]
pub struct CallExpr {
    pub callee: Box<Expr>, //ej: print("Hello World")
    pub arguments: Vec<Expr>,
}

#[derive(Clone)]
pub struct BaseCallExpr {
    pub arguments: Vec<Expr>,
}

#[derive(Clone)]
pub struct MacroCallExpr {
    pub name: String, //ej: reply(5)
    pub arguments: Vec<MacroCallArg>,
    pub action: Option<Box<Expr>>, // bloque trailing: repeat(10) { ... }
}

#[derive(Clone)]
pub struct MacroCallArg {
    pub kind: MacroParamKind,
    pub value: Expr,
}

#[derive(Clone)]
pub struct LetExpr {
    pub bindings: Vec<LetBinding>, //ej: let x =5, y = 10 in x+y
    pub body: Box<Expr>,
}

#[derive(Clone)]
pub struct LetBinding {
    pub name: String,
    pub types: Option<TypeRef>,
    pub initializer: Expr,
}

#[derive(Clone)]
pub enum TypeRef {
    Number,
    String,
    Boolean,
    Vector(Box<TypeRef>),                 // T[] - Vector type (postfix notation)
    Function(Vec<TypeRef>, Box<TypeRef>), // (T1, T2) -> Out - Function type
    Custom(String),                       // User-defined type
}

#[derive(Clone)]
pub struct BlockExpr {
    pub expressions: Vec<Expr>, //ej: { print(x); x + 1;}
}

#[derive(Clone)]
pub struct IfExpr {
    pub condition: Box<Expr>, //ej: if (x > 0) 1 else 0
    pub then_branch: Box<Expr>,
    pub elif_branches: Vec<(Expr, Expr)>, // Lista de (condición, cuerpo)
    pub else_branch: Box<Expr>,
}

#[derive(Clone)]
pub struct WhileExpr {
    pub condition: Box<Expr>, //ej: while (x < 10) { x = x + 1; }
    pub body: Box<Expr>,
}

#[derive(Clone)]
pub struct ForExpr {
    pub variable: String,
    pub iterable: Box<Expr>, //ej: for x in range(0,10) print(x)
    pub body: Box<Expr>,
}

#[derive(Clone)]
pub struct AssignExpr {
    pub target: Box<Expr>, //solo para :=
    pub value: Box<Expr>,  //ej: arr[0] := 42
}

#[derive(Clone)]
pub struct MemberAccessExpr {
    pub object: Box<Expr>, //ej: game.player.health
    pub field: String,
}

#[derive(Clone)]
pub struct IndexExpr {
    pub object: Box<Expr>, //ej: arr[0]
    pub index: Box<Expr>,
}

#[derive(Clone)]
pub struct ArrayExpr {
    pub elements: Vec<Expr>, //ej: [1, 2, 3]
}

#[derive(Clone)]
pub struct ArrayComprehensionExpr {
    pub element: Box<Expr>, // ej:[x * 2 || x in numbers]
    pub variable: String, // expresión generada (x * 2), variable iteradora (x), iterable (numbers)
    pub iterable: Box<Expr>,
}

#[derive(Clone)]
pub struct LambdaExpr {
    pub params: Vec<Param>,           //ej: (x) => x + 1
    pub return_type: Option<TypeRef>, //ej: (x: Number): Number => x + 1
    pub body: Box<Expr>,
}

#[derive(Clone)]
pub struct NewExpr {
    pub type_name: String, //ej: new Person("Ana")
    pub arguments: Vec<Expr>,
}

#[derive(Clone)]
pub struct IsExpr {
    pub expression: Box<Expr>, // x is Number
    pub type_info: TypeRef,
}

#[derive(Clone)]
pub struct AsExpr {
    pub expression: Box<Expr>, // x as Number
    pub type_info: TypeRef,
}
#[derive(Clone)]
pub struct MatchExpr {
    pub expression: Box<Expr>,
    pub cases: Vec<MatchCase>,
}

#[derive(Clone)]
pub struct MatchCase {
    pub pattern: Pattern,
    pub body: Expr,
}

#[derive(Clone)]
pub enum Pattern {
    Identifier {
        name: String,
        type_restriction: Option<TypeRef>,
    },
    Binary {
        left: Box<Pattern>,
        operator: BinaryOperator,
        right: Box<Pattern>,
    },
    Unary {
        operator: UnaryOperator,
        operand: Box<Pattern>,
    },
    Literal(LiteralValue),
    Default,
}
