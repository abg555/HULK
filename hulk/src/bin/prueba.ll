; ModuleID = 'prueba'
source_filename = "prueba"

%obj.Dog = type { double, double }

@print_fmt_num = private unnamed_addr constant [4 x i8] c"%f\0A\00", align 1

define double @main() {
entry:
  %obj_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Dog, ptr null, i32 1) to i64))
  %self = alloca ptr, align 8
  store ptr %obj_alloc, ptr %self, align 8
  %age_field = getelementptr inbounds %obj.Dog, ptr %obj_alloc, i32 0, i32 0
  store double 3.000000e+00, ptr %age_field, align 8
  %bark_field = getelementptr inbounds %obj.Dog, ptr %obj_alloc, i32 0, i32 1
  store double 7.000000e+00, ptr %bark_field, align 8
  %d = alloca ptr, align 8
  store ptr %obj_alloc, ptr %d, align 8
  %load_d = load ptr, ptr %d, align 8
  %age_field1 = getelementptr inbounds %obj.Dog, ptr %load_d, i32 0, i32 0
  %load_age = load double, ptr %age_field1, align 8
  %load_d2 = load ptr, ptr %d, align 8
  %bark_field3 = getelementptr inbounds %obj.Dog, ptr %load_d2, i32 0, i32 1
  %load_bark = load double, ptr %bark_field3, align 8
  %addtmp = fadd double %load_age, %load_bark
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_num, double %addtmp)
  ret double %addtmp
}

declare ptr @malloc(i64)

declare i32 @printf(ptr, ...)
