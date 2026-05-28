; ModuleID = 'prueba'
source_filename = "prueba"

%vtable.Base = type { i64 }
%vtable.Child = type { i64 }
%obj.Child = type { ptr, double, double }
%obj.Base = type { ptr, double }

@vtable.Base = constant %vtable.Base zeroinitializer
@vtable.Child = constant %vtable.Child { i64 1 }
@print_fmt_bool = private unnamed_addr constant [4 x i8] c"%d\0A\00", align 1
@as_panic_msg_0 = private unnamed_addr constant [32 x i8] c"Runtime error: cast 'as' failed\00", align 1
@print_fmt_obj = private unnamed_addr constant [8 x i8] c"<Child>\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@print_fmt_bool.1 = private unnamed_addr constant [4 x i8] c"%d\0A\00", align 1
@print_fmt_bool.2 = private unnamed_addr constant [4 x i8] c"%d\0A\00", align 1
@print_fmt_bool.3 = private unnamed_addr constant [4 x i8] c"%d\0A\00", align 1
@as_panic_msg_1 = private unnamed_addr constant [32 x i8] c"Runtime error: cast 'as' failed\00", align 1
@print_fmt_obj.4 = private unnamed_addr constant [8 x i8] c"<Child>\00", align 1
@print_fmt_str.5 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

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
  br i1 %as_cmp, label %as_ok, label %as_fail

as_ok:                                            ; preds = %entry
  br label %as_cont

as_fail:                                          ; preds = %entry
  call void @hulk_panic(ptr @as_panic_msg_0)
  unreachable

as_cont:                                          ; preds = %as_ok
  %print6 = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr @print_fmt_obj)
  %print7 = call i32 (ptr, ...) @printf(ptr @print_fmt_bool.1, i1 true)
  %print8 = call i32 (ptr, ...) @printf(ptr @print_fmt_bool.2, i1 true)
  %obj_alloc9 = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Base, ptr null, i32 1) to i64))
  %vtable_slot10 = getelementptr inbounds %obj.Base, ptr %obj_alloc9, i32 0, i32 0
  store ptr @vtable.Base, ptr %vtable_slot10, align 8
  %self11 = alloca ptr, align 8
  store ptr %obj_alloc9, ptr %self11, align 8
  %value_field12 = getelementptr inbounds %obj.Base, ptr %obj_alloc9, i32 0, i32 1
  store double 0.000000e+00, ptr %value_field12, align 8
  %y = alloca ptr, align 8
  store ptr %obj_alloc9, ptr %y, align 8
  %load_y = load ptr, ptr %y, align 8
  %vtable_slot13 = getelementptr inbounds %obj.Base, ptr %load_y, i32 0, i32 0
  %vtable_ptr14 = load ptr, ptr %vtable_slot13, align 8
  %type_id15 = load i64, ptr %vtable_ptr14, align 4
  %is_cmp16 = icmp eq i64 %type_id15, 1
  %print17 = call i32 (ptr, ...) @printf(ptr @print_fmt_bool.3, i1 %is_cmp16)
  %load_y18 = load ptr, ptr %y, align 8
  %vtable_slot19 = getelementptr inbounds %obj.Base, ptr %load_y18, i32 0, i32 0
  %vtable_ptr20 = load ptr, ptr %vtable_slot19, align 8
  %type_id21 = load i64, ptr %vtable_ptr20, align 4
  %as_cmp22 = icmp eq i64 %type_id21, 1
  br i1 %as_cmp22, label %as_ok23, label %as_fail24

as_ok23:                                          ; preds = %as_cont
  br label %as_cont25

as_fail24:                                        ; preds = %as_cont
  call void @hulk_panic(ptr @as_panic_msg_1)
  unreachable

as_cont25:                                        ; preds = %as_ok23
  %print26 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.5, ptr @print_fmt_obj.4)
  ret double 0.000000e+00
}

declare ptr @malloc(i64)

declare i32 @printf(ptr, ...)

declare void @hulk_panic(ptr)
