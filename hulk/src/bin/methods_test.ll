; ModuleID = 'methods_test'
source_filename = "methods_test"

%obj.Counter = type { double }

define double @Counter.increment(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  ret double 1.000000e+00
}

define double @main() {
entry:
  %obj_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Counter, ptr null, i32 1) to i64))
  %self = alloca ptr, align 8
  store ptr %obj_alloc, ptr %self, align 8
  %value_field = getelementptr inbounds %obj.Counter, ptr %obj_alloc, i32 0, i32 0
  store double 0.000000e+00, ptr %value_field, align 8
  %c = alloca ptr, align 8
  store ptr %obj_alloc, ptr %c, align 8
  %load_c = load ptr, ptr %c, align 8
  %call_Counter_increment = call double @Counter.increment(ptr %load_c)
  ret double %call_Counter_increment
}

declare ptr @malloc(i64)
