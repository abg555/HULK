#![allow(dead_code)]

pub struct Program {
    pub items: Vec<Item>, //nodo principal del arbol
}

pub enum Item {
    Function(FunctionDecl),
    Type(TypeDecl),
    Protocol(ProtocolDecl),
    Macro(MacroDecl),
    GlobalExpr(Expr),
}

pub struct FunctionDecl {
    pub name: String, //ej: function sum(x, y) => x + y;
    pub params: Vec<Param>,
    pub return_type: Option<TypeRef>,
    pub body: Expr,
}

pub struct Param {
    pub name: String,
    pub types: Option<TypeRef>,
}

pub struct TypeDecl {
    pub name: String, //ej: type Person{ name:String   mynameis()=> print(self.name)}
    pub param: Vec<Param>, //ej: type Point(x,y)...
    pub parent: Option<TypeRef>, //herencia
    pub parent_arg: Vec<Expr>, //parametros que se le pasan al padre
    pub fields: Vec<FieldDecl>,
    pub methods: Vec<FunctionDecl>,
}

pub struct FieldDecl {
    pub name: String,
    pub type_annotation: Option<TypeRef>,
    pub initializer: Expr,
}

pub struct ProtocolDecl {
    pub name: String, //ej: protocol Printable {print(): String}
    pub methods: Vec<ProtocolMethod>,
}

pub struct ProtocolMethod {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeRef,
}

pub struct MacroDecl {
    pub name: String,
    pub params: Vec<MacroParam>,
    pub body: Expr,
}

pub struct MacroParam {
    pub name: String,
    pub kind: MacroParamKind,
}

pub enum MacroParamKind {
    Normal,
    Block,       // *expr
    Symbolic,    // @x
    Placeholder, // $x
}

pub struct Expr {
    pub id: NodeId,     //id de la expresion
    pub span: Span, //para saber en que posicion se encuentra, util para debugueo y mostrar errores
    pub kind: KindExpr, //tipo de la expresion
}

pub struct NodeId(pub u32); //el campo id es de tipo NodeId que realmente es u32

pub struct Span {
    pub start: usize, //posiciones dentro de un string, y los strings en Rust se indexan con usize
    pub end: usize,
}

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

pub struct LiteralExpr {
    pub value: LiteralValue, //ej: 5, "hola mundo", true
}

pub enum LiteralValue {
    String(String),
    Number(f64),
    Bool(bool),
}

pub struct VariableExpr {
    pub name: String, //ej: x
}

pub struct BinaryExpr {
    pub left: Box<Expr>, //ej: 5+3
    pub operator: BinaryOperator,
    pub right: Box<Expr>,
}

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

pub struct UnaryExpr {
    pub operator: UnaryOperator, //ej: !true, -5
    pub right: Box<Expr>,
}

pub enum UnaryOperator {
    Negate,
    Not,
}

pub struct CallExpr {
    pub callee: Box<Expr>, //ej: print("Hello World")
    pub arguments: Vec<Expr>,
}

pub struct BaseCallExpr {
    pub arguments: Vec<Expr>,
}

pub struct MacroCallExpr {
    pub name: String, //ej: reply(5)
    pub arguments: Vec<Expr>,
}

pub struct LetExpr {
    pub bindings: Vec<LetBinding>, //ej: let x =5, y = 10 in x+y
    pub body: Box<Expr>,
}

pub struct LetBinding {
    pub name: String,
    pub types: Option<TypeRef>,
    pub initializer: Expr,
}

pub enum TypeRef {
    Number,
    String,
    Boolean,
    Array(Box<TypeRef>),
    Function(Vec<TypeRef>, Box<TypeRef>),
    Custom(String), // para tipos definidos por el usuario
}

pub struct BlockExpr {
    pub expressions: Vec<Expr>, //ej: { print(x); x + 1;}
}

pub struct IfExpr {
    pub condition: Box<Expr>, //ej: if (x > 0) 1 else 0
    pub then_branch: Box<Expr>,
    pub elif_branches: Vec<(Expr, Expr)>, // Lista de (condición, cuerpo)
    pub else_branch: Box<Expr>,
}

pub struct WhileExpr {
    pub condition: Box<Expr>, //ej: while (x < 10) { x = x + 1; }
    pub body: Box<Expr>,
}

pub struct ForExpr {
    pub variable: String,
    pub iterable: Box<Expr>, //ej: for x in range(0,10) print(x)
    pub body: Box<Expr>,
}

pub struct AssignExpr {
    pub target: Box<Expr>, //solo para :=
    pub value: Box<Expr>,  //ej: arr[0] := 42
}

pub struct MemberAccessExpr {
    pub object: Box<Expr>, //ej: game.player.health
    pub field: String,
}

pub struct IndexExpr {
    pub object: Box<Expr>, //ej: arr[0]
    pub index: Box<Expr>,
}

pub struct ArrayExpr {
    pub elements: Vec<Expr>, //ej: [1, 2, 3]
}

pub struct ArrayComprehensionExpr {
    pub element: Box<Expr>, // ej:[x * 2 || x in numbers]
    pub variable: String, // expresión generada (x * 2), variable iteradora (x), iterable (numbers)
    pub iterable: Box<Expr>,
}

pub struct LambdaExpr {
    pub params: Vec<Param>, //ej: (x) => x + 1
    pub return_type: Option<TypeRef>,
    pub body: Box<Expr>,
}

pub struct NewExpr {
    pub type_name: String, //ej: new Person("Ana")
    pub arguments: Vec<Expr>,
}

pub struct IsExpr {
    pub expression: Box<Expr>, // x is Number
    pub type_info: TypeRef,
}

pub struct AsExpr {
    pub expression: Box<Expr>, // x as Number
    pub type_info: TypeRef,
}
pub struct MatchExpr {
    pub expression: Box<Expr>,
    pub cases: Vec<MatchCase>,
}

pub struct MatchCase {
    pub pattern: Pattern,
    pub body: Expr,
}

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
