; ModuleID = 'prueba'
source_filename = "prueba"

@print_fmt_num = private unnamed_addr constant [4 x i8] c"%f\0A\00", align 1
@print_fmt_num.1 = private unnamed_addr constant [4 x i8] c"%f\0A\00", align 1
@print_fmt_num.2 = private unnamed_addr constant [4 x i8] c"%f\0A\00", align 1
@print_fmt_num.3 = private unnamed_addr constant [4 x i8] c"%f\0A\00", align 1

define double @operate(double %x, double %y) {
entry:
  %x1 = alloca double, align 8
  store double %x, ptr %x1, align 8
  %y2 = alloca double, align 8
  store double %y, ptr %y2, align 8
  %load_x = load double, ptr %x1, align 8
  %load_y = load double, ptr %y2, align 8
  %addtmp = fadd double %load_x, %load_y
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_num, double %addtmp)
  %load_x3 = load double, ptr %x1, align 8
  %load_y4 = load double, ptr %y2, align 8
  %subtmp = fsub double %load_x3, %load_y4
  %print5 = call i32 (ptr, ...) @printf(ptr @print_fmt_num.1, double %subtmp)
  %load_x6 = load double, ptr %x1, align 8
  %load_y7 = load double, ptr %y2, align 8
  %multmp = fmul double %load_x6, %load_y7
  %print8 = call i32 (ptr, ...) @printf(ptr @print_fmt_num.2, double %multmp)
  %load_x9 = load double, ptr %x1, align 8
  %load_y10 = load double, ptr %y2, align 8
  %divtmp = fdiv double %load_x9, %load_y10
  %print11 = call i32 (ptr, ...) @printf(ptr @print_fmt_num.3, double %divtmp)
  ret double %divtmp
}

declare i32 @printf(ptr, ...)

define double @main() {
entry:
  %call_operate = call double @operate(double 1.000000e+01, double 5.000000e+00)
  ret double %call_operate
}
