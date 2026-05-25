; ModuleID = 'prueba'
source_filename = "prueba"

%obj.Point = type { double, double }

@str = private unnamed_addr constant [4 x i8] c"x: \00", align 1
@str.1 = private unnamed_addr constant [6 x i8] c"; y: \00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define double @Point.getX(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %load_self = load ptr, ptr %self1, align 8
  %x_field = getelementptr inbounds %obj.Point, ptr %load_self, i32 0, i32 0
  %load_x = load double, ptr %x_field, align 8
  ret double %load_x
}

define double @Point.getY(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %load_self = load ptr, ptr %self1, align 8
  %y_field = getelementptr inbounds %obj.Point, ptr %load_self, i32 0, i32 1
  %load_y = load double, ptr %y_field, align 8
  ret double %load_y
}

define double @Point.setX(ptr %self, double %new_x) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %new_x2 = alloca double, align 8
  store double %new_x, ptr %new_x2, align 8
  %load_self = load ptr, ptr %self1, align 8
  %load_new_x = load double, ptr %new_x2, align 8
  %x_field = getelementptr inbounds %obj.Point, ptr %load_self, i32 0, i32 0
  store double %load_new_x, ptr %x_field, align 8
  %reload_x = load double, ptr %x_field, align 8
  ret double %reload_x
}

define double @Point.setY(ptr %self, double %new_y) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %new_y2 = alloca double, align 8
  store double %new_y, ptr %new_y2, align 8
  %load_self = load ptr, ptr %self1, align 8
  %load_new_y = load double, ptr %new_y2, align 8
  %y_field = getelementptr inbounds %obj.Point, ptr %load_self, i32 0, i32 1
  store double %load_new_y, ptr %y_field, align 8
  %reload_y = load double, ptr %y_field, align 8
  ret double %reload_y
}

define double @main() {
entry:
  %obj_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Point, ptr null, i32 1) to i64))
  %self = alloca ptr, align 8
  store ptr %obj_alloc, ptr %self, align 8
  %x0 = alloca double, align 8
  store double 3.000000e+00, ptr %x0, align 8
  %y0 = alloca double, align 8
  store double 4.000000e+00, ptr %y0, align 8
  %load_x0 = load double, ptr %x0, align 8
  %x_field = getelementptr inbounds %obj.Point, ptr %obj_alloc, i32 0, i32 0
  store double %load_x0, ptr %x_field, align 8
  %load_y0 = load double, ptr %y0, align 8
  %y_field = getelementptr inbounds %obj.Point, ptr %obj_alloc, i32 0, i32 1
  store double %load_y0, ptr %y_field, align 8
  %pt = alloca ptr, align 8
  store ptr %obj_alloc, ptr %pt, align 8
  %load_pt = load ptr, ptr %pt, align 8
  %call_Point_getX = call double @Point.getX(ptr %load_pt)
  %format_num = call ptr @hulk_format_number(double %call_Point_getX)
  %concat = call ptr @hulk_concat(ptr @str, ptr %format_num)
  %concat1 = call ptr @hulk_concat(ptr %concat, ptr @str.1)
  %load_pt2 = load ptr, ptr %pt, align 8
  %call_Point_getY = call double @Point.getY(ptr %load_pt2)
  %format_num3 = call ptr @hulk_format_number(double %call_Point_getY)
  %concat4 = call ptr @hulk_concat(ptr %concat1, ptr %format_num3)
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr %concat4)
  ret double 0.000000e+00
}

declare ptr @malloc(i64)

declare ptr @hulk_format_number(double)

declare ptr @hulk_concat(ptr, ptr)

declare i32 @printf(ptr, ...)
