; ModuleID = 'methods_test'
source_filename = "methods_test"

%vtable.Counter = type { i64, ptr }
%obj.Counter = type { ptr, double }

@vtable.Counter = constant %vtable.Counter { i64 0, ptr @Counter.increment }

define double @Counter.increment(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  ret double 1.000000e+00
}

define double @main() {
entry:
  %obj_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Counter, ptr null, i32 1) to i64))
  %vtable_slot = getelementptr inbounds %obj.Counter, ptr %obj_alloc, i32 0, i32 0
  store ptr @vtable.Counter, ptr %vtable_slot, align 8
  %self = alloca ptr, align 8
  store ptr %obj_alloc, ptr %self, align 8
  %value_field = getelementptr inbounds %obj.Counter, ptr %obj_alloc, i32 0, i32 1
  store double 0.000000e+00, ptr %value_field, align 8
  %c = alloca ptr, align 8
  store ptr %obj_alloc, ptr %c, align 8
  %load_c = load ptr, ptr %c, align 8
  %vtable_ptr = getelementptr inbounds %obj.Counter, ptr %load_c, i32 0, i32 0
  %vtable_load = load ptr, ptr %vtable_ptr, align 8
  %method_ptr = getelementptr inbounds %vtable.Counter, ptr %vtable_load, i32 0, i32 1
  %method_load = load ptr, ptr %method_ptr, align 8
  %call_Counter_increment = call double %method_load(ptr %load_c)
  ret double %call_Counter_increment
}

declare ptr @malloc(i64)
