; ModuleID = 'prueba'
source_filename = "prueba"

%vtable.Base = type { i64 }
%vtable.Child = type { i64 }
%obj.Child = type { ptr, double, double }
%obj.Base = type { ptr, double }

@vtable.Base = constant %vtable.Base zeroinitializer
@vtable.Child = constant %vtable.Child { i64 1 }
@print_fmt_bool = private unnamed_addr constant [4 x i8] c"%d\0A\00", align 1
@print_fmt_obj = private unnamed_addr constant [8 x i8] c"<Child>\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define double @main() {
entry:
  %obj_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Child, ptr null, i32 1) to i64))
  %vtable_slot = getelementptr inbounds %obj.Child, ptr %obj_alloc, i32 0, i32 0
  store ptr @vtable.Child, ptr %vtable_slot, align 8
  %self = alloca ptr, align 8
  store ptr %obj_alloc, ptr %self, align 8
  %value_field = getelementptr inbounds %obj.Child, ptr %obj_alloc, i32 0, i32 1
  store double 0.000000e+00, ptr %value_field, align 8
  %child_value_field = getelementptr inbounds %obj.Child, ptr %obj_alloc, i32 0, i32 2
  store double 1.000000e+00, ptr %child_value_field, align 8
  %x = alloca ptr, align 8
  store ptr %obj_alloc, ptr %x, align 8
  %load_x = load ptr, ptr %x, align 8
  %vtable_slot1 = getelementptr inbounds %obj.Base, ptr %load_x, i32 0, i32 0
  %vtable_ptr = load ptr, ptr %vtable_slot1, align 8
  %type_id = load i64, ptr %vtable_ptr, align 4
  %is_cmp = icmp eq i64 %type_id, 1
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_bool, i1 %is_cmp)
  %load_x2 = load ptr, ptr %x, align 8
  %vtable_slot3 = getelementptr inbounds %obj.Base, ptr %load_x2, i32 0, i32 0
  %vtable_ptr4 = load ptr, ptr %vtable_slot3, align 8
  %type_id5 = load i64, ptr %vtable_ptr4, align 4
  %as_cmp = icmp eq i64 %type_id5, 1
  %as_value = select i1 %as_cmp, ptr %load_x2, ptr null
  %print6 = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr @print_fmt_obj)
  ret double 0.000000e+00
}

declare ptr @malloc(i64)

declare i32 @printf(ptr, ...)
