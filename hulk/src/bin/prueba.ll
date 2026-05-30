; ModuleID = 'prueba'
source_filename = "prueba"

%vtable.Point = type { i64, ptr, ptr, ptr, ptr }
%obj.Point = type { ptr, double, double }

@vtable.Point = constant %vtable.Point { i64 0, ptr @Point.getX, ptr @Point.getY, ptr @Point.setX, ptr @Point.setY }
@str = private unnamed_addr constant [4 x i8] c"x: \00", align 1
@str.1 = private unnamed_addr constant [6 x i8] c"; y: \00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

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

define double @Point.setX(ptr %self, double %x) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %x2 = alloca double, align 8
  store double %x, ptr %x2, align 8
  %load_self = load ptr, ptr %self1, align 8
  %load_x = load double, ptr %x2, align 8
  %x_field = getelementptr inbounds %obj.Point, ptr %load_self, i32 0, i32 0
  store double %load_x, ptr %x_field, align 8
  %reload_x = load double, ptr %x_field, align 8
  ret double %reload_x
}

define double @Point.setY(ptr %self, double %y) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %y2 = alloca double, align 8
  store double %y, ptr %y2, align 8
  %load_self = load ptr, ptr %self1, align 8
  %load_y = load double, ptr %y2, align 8
  %y_field = getelementptr inbounds %obj.Point, ptr %load_self, i32 0, i32 1
  store double %load_y, ptr %y_field, align 8
  %reload_y = load double, ptr %y_field, align 8
  ret double %reload_y
}

define double @main() {
entry:
  %obj_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Point, ptr null, i32 1) to i64))
  %vtable_slot = getelementptr inbounds %obj.Point, ptr %obj_alloc, i32 0, i32 0
  store ptr @vtable.Point, ptr %vtable_slot, align 8
  %self = alloca ptr, align 8
  store ptr %obj_alloc, ptr %self, align 8
  %x_field = getelementptr inbounds %obj.Point, ptr %obj_alloc, i32 0, i32 1
  store double 3.000000e+00, ptr %x_field, align 8
  %y_field = getelementptr inbounds %obj.Point, ptr %obj_alloc, i32 0, i32 2
  store double 0.000000e+00, ptr %y_field, align 8
  %pt = alloca ptr, align 8
  store ptr %obj_alloc, ptr %pt, align 8
  %load_pt = load ptr, ptr %pt, align 8
  %vtable_ptr = getelementptr inbounds %obj.Point, ptr %load_pt, i32 0, i32 0
  %vtable_load = load ptr, ptr %vtable_ptr, align 8
  %method_ptr = getelementptr inbounds %vtable.Point, ptr %vtable_load, i32 0, i32 1
  %method_load = load ptr, ptr %method_ptr, align 8
  %call_Point_getX = call double %method_load(ptr %load_pt)
  %format_num = call ptr @hulk_format_number(double %call_Point_getX)
  %concat = call ptr @hulk_concat(ptr @str, ptr %format_num)
  %concat1 = call ptr @hulk_concat(ptr %concat, ptr @str.1)
  %load_pt2 = load ptr, ptr %pt, align 8
  %vtable_ptr3 = getelementptr inbounds %obj.Point, ptr %load_pt2, i32 0, i32 0
  %vtable_load4 = load ptr, ptr %vtable_ptr3, align 8
  %method_ptr5 = getelementptr inbounds %vtable.Point, ptr %vtable_load4, i32 0, i32 2
  %method_load6 = load ptr, ptr %method_ptr5, align 8
  %call_Point_getY = call double %method_load6(ptr %load_pt2)
  %format_num7 = call ptr @hulk_format_number(double %call_Point_getY)
  %concat8 = call ptr @hulk_concat(ptr %concat1, ptr %format_num7)
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr %concat8)
  ret double 0.000000e+00
}

declare ptr @malloc(i64)

declare ptr @hulk_format_number(double)

declare ptr @hulk_concat(ptr, ptr)

declare i32 @printf(ptr, ...)
