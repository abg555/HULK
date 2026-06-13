; ModuleID = 'tests'
source_filename = "tests"

%vtable.Point = type { i64, ptr, ptr, ptr }
%obj.Point = type { ptr, double, double }

@vtable.Point = constant %vtable.Point { i64 0, ptr @Point.getX, ptr @Point.getY, ptr @Point.sum }
@str = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.1 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.2 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.3 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.4 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.5 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.6 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.7 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.8 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.9 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.10 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define double @Point.getX(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %load_self = load ptr, ptr %self1, align 8
  %x_field = getelementptr inbounds %obj.Point, ptr %load_self, i32 0, i32 1
  %load_x = load double, ptr %x_field, align 8
  ret double %load_x
}

define double @Point.getY(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %load_self = load ptr, ptr %self1, align 8
  %y_field = getelementptr inbounds %obj.Point, ptr %load_self, i32 0, i32 2
  %load_y = load double, ptr %y_field, align 8
  ret double %load_y
}

define double @Point.sum(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %load_self = load ptr, ptr %self1, align 8
  %x_field = getelementptr inbounds %obj.Point, ptr %load_self, i32 0, i32 1
  %load_x = load double, ptr %x_field, align 8
  %load_self2 = load ptr, ptr %self1, align 8
  %y_field = getelementptr inbounds %obj.Point, ptr %load_self2, i32 0, i32 2
  %load_y = load double, ptr %y_field, align 8
  %addtmp = fadd double %load_x, %load_y
  ret double %addtmp
}

define i32 @main() {
entry:
  %obj_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Point, ptr null, i32 1) to i64))
  %vtable_slot = getelementptr inbounds %obj.Point, ptr %obj_alloc, i32 0, i32 0
  store ptr @vtable.Point, ptr %vtable_slot, align 8
  %self = alloca ptr, align 8
  store ptr %obj_alloc, ptr %self, align 8
  %x_val = alloca double, align 8
  store double 3.000000e+00, ptr %x_val, align 8
  %y_val = alloca double, align 8
  store double 4.000000e+00, ptr %y_val, align 8
  %load_x_val = load double, ptr %x_val, align 8
  %x_field = getelementptr inbounds %obj.Point, ptr %obj_alloc, i32 0, i32 1
  store double %load_x_val, ptr %x_field, align 8
  %load_y_val = load double, ptr %y_val, align 8
  %y_field = getelementptr inbounds %obj.Point, ptr %obj_alloc, i32 0, i32 2
  store double %load_y_val, ptr %y_field, align 8
  %p = alloca ptr, align 8
  store ptr %obj_alloc, ptr %p, align 8
  %load_p = load ptr, ptr %p, align 8
  %vtable_ptr = getelementptr inbounds %obj.Point, ptr %load_p, i32 0, i32 0
  %vtable_load = load ptr, ptr %vtable_ptr, align 8
  %method_ptr = getelementptr inbounds %vtable.Point, ptr %vtable_load, i32 0, i32 1
  %method_load = load ptr, ptr %method_ptr, align 8
  %call_Point_getX = call double %method_load(ptr %load_p)
  %cmptmp = fcmp oeq double %call_Point_getX, 3.000000e+00
  br i1 %cmptmp, label %if_then, label %if_else

if_then:                                          ; preds = %entry
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr @str)
  br label %if_merge

if_else:                                          ; preds = %entry
  %print1 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.2, ptr @str.1)
  br label %if_merge

if_merge:                                         ; preds = %if_else, %if_then
  %iftmp_str = phi ptr [ @str, %if_then ], [ @str.1, %if_else ]
  %load_p2 = load ptr, ptr %p, align 8
  %vtable_ptr3 = getelementptr inbounds %obj.Point, ptr %load_p2, i32 0, i32 0
  %vtable_load4 = load ptr, ptr %vtable_ptr3, align 8
  %method_ptr5 = getelementptr inbounds %vtable.Point, ptr %vtable_load4, i32 0, i32 2
  %method_load6 = load ptr, ptr %method_ptr5, align 8
  %call_Point_getY = call double %method_load6(ptr %load_p2)
  %cmptmp7 = fcmp oeq double %call_Point_getY, 4.000000e+00
  br i1 %cmptmp7, label %if_then8, label %if_else9

if_then8:                                         ; preds = %if_merge
  %print11 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.4, ptr @str.3)
  br label %if_merge10

if_else9:                                         ; preds = %if_merge
  %print12 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.6, ptr @str.5)
  br label %if_merge10

if_merge10:                                       ; preds = %if_else9, %if_then8
  %iftmp_str13 = phi ptr [ @str.3, %if_then8 ], [ @str.5, %if_else9 ]
  %load_p14 = load ptr, ptr %p, align 8
  %vtable_ptr15 = getelementptr inbounds %obj.Point, ptr %load_p14, i32 0, i32 0
  %vtable_load16 = load ptr, ptr %vtable_ptr15, align 8
  %method_ptr17 = getelementptr inbounds %vtable.Point, ptr %vtable_load16, i32 0, i32 3
  %method_load18 = load ptr, ptr %method_ptr17, align 8
  %call_Point_sum = call double %method_load18(ptr %load_p14)
  %cmptmp19 = fcmp oeq double %call_Point_sum, 7.000000e+00
  br i1 %cmptmp19, label %if_then20, label %if_else21

if_then20:                                        ; preds = %if_merge10
  %print23 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.8, ptr @str.7)
  br label %if_merge22

if_else21:                                        ; preds = %if_merge10
  %print24 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.10, ptr @str.9)
  br label %if_merge22

if_merge22:                                       ; preds = %if_else21, %if_then20
  %iftmp_str25 = phi ptr [ @str.7, %if_then20 ], [ @str.9, %if_else21 ]
  ret i32 0
}

declare ptr @malloc(i64)

declare i32 @printf(ptr, ...)
