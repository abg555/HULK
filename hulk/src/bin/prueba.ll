; ModuleID = 'prueba'
source_filename = "prueba"

%vtable.Point = type { i64, ptr, ptr }
%vtable.PolarPoint = type { i64, ptr, ptr, ptr }
%vtable.PolarPoint2 = type { i64, ptr, ptr, ptr }
%obj.Point = type { ptr, double, double }
%obj.PolarPoint = type { ptr, double, double }
%obj.PolarPoint2 = type { ptr, double, double, double }

@vtable.Point = constant %vtable.Point { i64 0, ptr @Point.getX, ptr @Point.getY }
@vtable.PolarPoint = constant %vtable.PolarPoint { i64 1, ptr @Point.getX, ptr @Point.getY, ptr @PolarPoint.rho }
@vtable.PolarPoint2 = constant %vtable.PolarPoint2 { i64 2, ptr @Point.getX, ptr @Point.getY, ptr @PolarPoint2.rho2 }
@str = private unnamed_addr constant [6 x i8] c"rho: \00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.1 = private unnamed_addr constant [7 x i8] c"rho2: \00", align 1
@print_fmt_str.2 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

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

define double @PolarPoint.rho(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %load_self = load ptr, ptr %self1, align 8
  %vtable_ptr = getelementptr inbounds %obj.PolarPoint, ptr %load_self, i32 0, i32 0
  %vtable_load = load ptr, ptr %vtable_ptr, align 8
  %method_ptr = getelementptr inbounds %vtable.PolarPoint, ptr %vtable_load, i32 0, i32 1
  %method_load = load ptr, ptr %method_ptr, align 8
  %call_Point_getX = call double %method_load(ptr %load_self)
  %powtmp = call double @llvm.pow.f64(double %call_Point_getX, double 2.000000e+00)
  %load_self2 = load ptr, ptr %self1, align 8
  %vtable_ptr3 = getelementptr inbounds %obj.PolarPoint, ptr %load_self2, i32 0, i32 0
  %vtable_load4 = load ptr, ptr %vtable_ptr3, align 8
  %method_ptr5 = getelementptr inbounds %vtable.PolarPoint, ptr %vtable_load4, i32 0, i32 2
  %method_load6 = load ptr, ptr %method_ptr5, align 8
  %call_Point_getY = call double %method_load6(ptr %load_self2)
  %powtmp7 = call double @llvm.pow.f64(double %call_Point_getY, double 2.000000e+00)
  %addtmp = fadd double %powtmp, %powtmp7
  %llvm.sqrt.f64 = call double @llvm.sqrt.f64(double %addtmp)
  ret double %llvm.sqrt.f64
}

define double @PolarPoint2.rho2(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %load_self = load ptr, ptr %self1, align 8
  %rho_saved_field = getelementptr inbounds %obj.PolarPoint2, ptr %load_self, i32 0, i32 3
  %load_rho_saved = load double, ptr %rho_saved_field, align 8
  ret double %load_rho_saved
}

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.pow.f64(double, double) #0

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.sqrt.f64(double) #0

define double @main() {
entry:
  %obj_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.PolarPoint, ptr null, i32 1) to i64))
  %vtable_slot = getelementptr inbounds %obj.PolarPoint, ptr %obj_alloc, i32 0, i32 0
  store ptr @vtable.PolarPoint, ptr %vtable_slot, align 8
  %self = alloca ptr, align 8
  store ptr %obj_alloc, ptr %self, align 8
  %x = alloca double, align 8
  store double 3.000000e+00, ptr %x, align 8
  %y = alloca double, align 8
  store double 4.000000e+00, ptr %y, align 8
  %load_x = load double, ptr %x, align 8
  %x1 = alloca double, align 8
  store double %load_x, ptr %x1, align 8
  %load_y = load double, ptr %y, align 8
  %y2 = alloca double, align 8
  store double %load_y, ptr %y2, align 8
  %load_x3 = load double, ptr %x1, align 8
  %x_field = getelementptr inbounds %obj.PolarPoint, ptr %obj_alloc, i32 0, i32 1
  store double %load_x3, ptr %x_field, align 8
  %load_y4 = load double, ptr %y2, align 8
  %y_field = getelementptr inbounds %obj.PolarPoint, ptr %obj_alloc, i32 0, i32 2
  store double %load_y4, ptr %y_field, align 8
  %p = alloca ptr, align 8
  store ptr %obj_alloc, ptr %p, align 8
  %load_p = load ptr, ptr %p, align 8
  %vtable_ptr = getelementptr inbounds %obj.PolarPoint, ptr %load_p, i32 0, i32 0
  %vtable_load = load ptr, ptr %vtable_ptr, align 8
  %method_ptr = getelementptr inbounds %vtable.PolarPoint, ptr %vtable_load, i32 0, i32 3
  %method_load = load ptr, ptr %method_ptr, align 8
  %call_PolarPoint_rho = call double %method_load(ptr %load_p)
  %format_num = call ptr @hulk_format_number(double %call_PolarPoint_rho)
  %concat = call ptr @hulk_concat(ptr @str, ptr %format_num)
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr %concat)
  %obj_alloc5 = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.PolarPoint2, ptr null, i32 1) to i64))
  %vtable_slot6 = getelementptr inbounds %obj.PolarPoint2, ptr %obj_alloc5, i32 0, i32 0
  store ptr @vtable.PolarPoint2, ptr %vtable_slot6, align 8
  %self7 = alloca ptr, align 8
  store ptr %obj_alloc5, ptr %self7, align 8
  %phi = alloca double, align 8
  store double 1.000000e+00, ptr %phi, align 8
  %rho = alloca double, align 8
  store double 2.000000e+00, ptr %rho, align 8
  %load_rho = load double, ptr %rho, align 8
  %load_phi = load double, ptr %phi, align 8
  %llvm.sin.f64 = call double @llvm.sin.f64(double %load_phi)
  %multmp = fmul double %load_rho, %llvm.sin.f64
  %x8 = alloca double, align 8
  store double %multmp, ptr %x8, align 8
  %load_rho9 = load double, ptr %rho, align 8
  %load_phi10 = load double, ptr %phi, align 8
  %llvm.cos.f64 = call double @llvm.cos.f64(double %load_phi10)
  %multmp11 = fmul double %load_rho9, %llvm.cos.f64
  %y12 = alloca double, align 8
  store double %multmp11, ptr %y12, align 8
  %load_x13 = load double, ptr %x8, align 8
  %x_field14 = getelementptr inbounds %obj.PolarPoint2, ptr %obj_alloc5, i32 0, i32 1
  store double %load_x13, ptr %x_field14, align 8
  %load_y15 = load double, ptr %y12, align 8
  %y_field16 = getelementptr inbounds %obj.PolarPoint2, ptr %obj_alloc5, i32 0, i32 2
  store double %load_y15, ptr %y_field16, align 8
  %load_rho17 = load double, ptr %rho, align 8
  %rho_saved_field = getelementptr inbounds %obj.PolarPoint2, ptr %obj_alloc5, i32 0, i32 3
  store double %load_rho17, ptr %rho_saved_field, align 8
  %q = alloca ptr, align 8
  store ptr %obj_alloc5, ptr %q, align 8
  %load_q = load ptr, ptr %q, align 8
  %vtable_ptr18 = getelementptr inbounds %obj.PolarPoint2, ptr %load_q, i32 0, i32 0
  %vtable_load19 = load ptr, ptr %vtable_ptr18, align 8
  %method_ptr20 = getelementptr inbounds %vtable.PolarPoint2, ptr %vtable_load19, i32 0, i32 3
  %method_load21 = load ptr, ptr %method_ptr20, align 8
  %call_PolarPoint2_rho2 = call double %method_load21(ptr %load_q)
  %format_num22 = call ptr @hulk_format_number(double %call_PolarPoint2_rho2)
  %concat23 = call ptr @hulk_concat(ptr @str.1, ptr %format_num22)
  %print24 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.2, ptr %concat23)
  ret double 0.000000e+00
}

declare ptr @malloc(i64)

declare ptr @hulk_format_number(double)

declare ptr @hulk_concat(ptr, ptr)

declare i32 @printf(ptr, ...)

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.sin.f64(double) #0

; Function Attrs: nocallback nofree nosync nounwind speculatable willreturn memory(none)
declare double @llvm.cos.f64(double) #0

attributes #0 = { nocallback nofree nosync nounwind speculatable willreturn memory(none) }
