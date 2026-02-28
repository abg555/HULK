#![allow(dead_code)]

pub struct Expr {
    pub id: NodeId,     //id de la expresion
    pub spand: Spand, //para saber en que posicion se encuentra, util para debugueo y mostrar errores
    pub kind: KindExpr, //tipo de la expresion
}

pub struct NodeId(pub u32); //el campo id es de tipo NodeId que realmente es u32

pub struct Spand {
    pub start: usize, //posiciones dentro de un string, y los strings en Rust se indexan con usize
    pub end: usize,
}

pub enum KindExpr {
    Literal(LiteralExpr),
    Variable(VariableExpr),
    Binary(BinaryExpr),
    Unary(UnaryExpr),
    Call(CallExpr),
    Let(LetExpr),
    Block(BlockExpr),
    If(IfExpr),
    While(WhileExpr),
    For(ForExpr),
    Assign(AssignExpr),
    MemberAccess(MemberAccessExpr),
    Index(IndexExpr),
    Array(ArrayExpr),
    Object(ObjectExpr),
    Lambda(LambdaExpr),
    New(NewExpr),
    Is(IsExpr),
    As(AsExpr),
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

pub struct ObjectExpr {
    pub fields: Vec<(String, Expr)>, //ej: { name: "Ana", age: 20 }
}

pub struct LambdaExpr {
    pub params: Vec<String>, //ej: (x) => x + 1
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
