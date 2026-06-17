; ModuleID = 'tests'
source_filename = "tests"

%vtable.Temperature = type { i64, ptr }
%obj.Temperature = type { ptr, double }

@vtable.Temperature = constant %vtable.Temperature { i64 0, ptr @Temperature.value }
@proto_panic_msg_0 = private unnamed_addr constant [68 x i8] c"Runtime error: tipo desconocido en dispatch de protocolo Comparable\00", align 1
@proto_panic_msg_1 = private unnamed_addr constant [68 x i8] c"Runtime error: tipo desconocido en dispatch de protocolo Comparable\00", align 1
@proto_panic_msg_2 = private unnamed_addr constant [68 x i8] c"Runtime error: tipo desconocido en dispatch de protocolo Comparable\00", align 1
@proto_panic_msg_3 = private unnamed_addr constant [68 x i8] c"Runtime error: tipo desconocido en dispatch de protocolo Comparable\00", align 1
@str = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.1 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.2 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.3 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.4 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.5 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.6 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define double @max_comp(ptr %a, ptr %b) {
entry:
  %a1 = alloca ptr, align 8
  store ptr %a, ptr %a1, align 8
  %b2 = alloca ptr, align 8
  store ptr %b, ptr %b2, align 8
  %load_a = load ptr, ptr %a1, align 8
  %proto_vtable_slot = getelementptr inbounds %obj.Temperature, ptr %load_a, i32 0, i32 0
  %proto_vtable_ptr = load ptr, ptr %proto_vtable_slot, align 8
  %proto_type_id = load i64, ptr %proto_vtable_ptr, align 4
  %proto_result = alloca double, align 8
  store double 0.000000e+00, ptr %proto_result, align 8
  br label %proto_dispatch

proto_after:                                      ; preds = %proto_match_Temperature
  %proto_result_load = load double, ptr %proto_result, align 8
  %load_b = load ptr, ptr %b2, align 8
  %proto_vtable_slot3 = getelementptr inbounds %obj.Temperature, ptr %load_b, i32 0, i32 0
  %proto_vtable_ptr4 = load ptr, ptr %proto_vtable_slot3, align 8
  %proto_type_id5 = load i64, ptr %proto_vtable_ptr4, align 4
  %proto_result6 = alloca double, align 8
  store double 0.000000e+00, ptr %proto_result6, align 8
  br label %proto_dispatch8

proto_dispatch:                                   ; preds = %entry
  %proto_cmp = icmp eq i64 %proto_type_id, 0
  br i1 %proto_cmp, label %proto_match_Temperature, label %proto_next_Temperature

proto_match_Temperature:                          ; preds = %proto_dispatch
  %vtable_slot = getelementptr inbounds %obj.Temperature, ptr %load_a, i32 0, i32 0
  %vtable_load = load ptr, ptr %vtable_slot, align 8
  %method_slot = getelementptr inbounds %vtable.Temperature, ptr %vtable_load, i32 0, i32 1
  %method_load = load ptr, ptr %method_slot, align 8
  %call_vtable = call double %method_load(ptr %load_a)
  store double %call_vtable, ptr %proto_result, align 8
  br label %proto_after

proto_next_Temperature:                           ; preds = %proto_dispatch
  call void @hulk_panic(ptr @proto_panic_msg_0)
  unreachable

proto_after7:                                     ; preds = %proto_match_Temperature10
  %proto_result_load17 = load double, ptr %proto_result6, align 8
  %cmptmp = fcmp oge double %proto_result_load, %proto_result_load17
  br i1 %cmptmp, label %if_then, label %if_else

proto_dispatch8:                                  ; preds = %proto_after
  %proto_cmp9 = icmp eq i64 %proto_type_id5, 0
  br i1 %proto_cmp9, label %proto_match_Temperature10, label %proto_next_Temperature11

proto_match_Temperature10:                        ; preds = %proto_dispatch8
  %vtable_slot12 = getelementptr inbounds %obj.Temperature, ptr %load_b, i32 0, i32 0
  %vtable_load13 = load ptr, ptr %vtable_slot12, align 8
  %method_slot14 = getelementptr inbounds %vtable.Temperature, ptr %vtable_load13, i32 0, i32 1
  %method_load15 = load ptr, ptr %method_slot14, align 8
  %call_vtable16 = call double %method_load15(ptr %load_b)
  store double %call_vtable16, ptr %proto_result6, align 8
  br label %proto_after7

proto_next_Temperature11:                         ; preds = %proto_dispatch8
  call void @hulk_panic(ptr @proto_panic_msg_1)
  unreachable

if_then:                                          ; preds = %proto_after7
  %load_a18 = load ptr, ptr %a1, align 8
  %proto_vtable_slot19 = getelementptr inbounds %obj.Temperature, ptr %load_a18, i32 0, i32 0
  %proto_vtable_ptr20 = load ptr, ptr %proto_vtable_slot19, align 8
  %proto_type_id21 = load i64, ptr %proto_vtable_ptr20, align 4
  %proto_result22 = alloca double, align 8
  store double 0.000000e+00, ptr %proto_result22, align 8
  br label %proto_dispatch24

if_else:                                          ; preds = %proto_after7
  %load_b34 = load ptr, ptr %b2, align 8
  %proto_vtable_slot35 = getelementptr inbounds %obj.Temperature, ptr %load_b34, i32 0, i32 0
  %proto_vtable_ptr36 = load ptr, ptr %proto_vtable_slot35, align 8
  %proto_type_id37 = load i64, ptr %proto_vtable_ptr36, align 4
  %proto_result38 = alloca double, align 8
  store double 0.000000e+00, ptr %proto_result38, align 8
  br label %proto_dispatch40

if_merge:                                         ; preds = %proto_after39, %proto_after23
  %iftmp = phi double [ %proto_result_load33, %proto_after23 ], [ %proto_result_load49, %proto_after39 ]
  ret double %iftmp

proto_after23:                                    ; preds = %proto_match_Temperature26
  %proto_result_load33 = load double, ptr %proto_result22, align 8
  br label %if_merge

proto_dispatch24:                                 ; preds = %if_then
  %proto_cmp25 = icmp eq i64 %proto_type_id21, 0
  br i1 %proto_cmp25, label %proto_match_Temperature26, label %proto_next_Temperature27

proto_match_Temperature26:                        ; preds = %proto_dispatch24
  %vtable_slot28 = getelementptr inbounds %obj.Temperature, ptr %load_a18, i32 0, i32 0
  %vtable_load29 = load ptr, ptr %vtable_slot28, align 8
  %method_slot30 = getelementptr inbounds %vtable.Temperature, ptr %vtable_load29, i32 0, i32 1
  %method_load31 = load ptr, ptr %method_slot30, align 8
  %call_vtable32 = call double %method_load31(ptr %load_a18)
  store double %call_vtable32, ptr %proto_result22, align 8
  br label %proto_after23

proto_next_Temperature27:                         ; preds = %proto_dispatch24
  call void @hulk_panic(ptr @proto_panic_msg_2)
  unreachable

proto_after39:                                    ; preds = %proto_match_Temperature42
  %proto_result_load49 = load double, ptr %proto_result38, align 8
  br label %if_merge

proto_dispatch40:                                 ; preds = %if_else
  %proto_cmp41 = icmp eq i64 %proto_type_id37, 0
  br i1 %proto_cmp41, label %proto_match_Temperature42, label %proto_next_Temperature43

proto_match_Temperature42:                        ; preds = %proto_dispatch40
  %vtable_slot44 = getelementptr inbounds %obj.Temperature, ptr %load_b34, i32 0, i32 0
  %vtable_load45 = load ptr, ptr %vtable_slot44, align 8
  %method_slot46 = getelementptr inbounds %vtable.Temperature, ptr %vtable_load45, i32 0, i32 1
  %method_load47 = load ptr, ptr %method_slot46, align 8
  %call_vtable48 = call double %method_load47(ptr %load_b34)
  store double %call_vtable48, ptr %proto_result38, align 8
  br label %proto_after39

proto_next_Temperature43:                         ; preds = %proto_dispatch40
  call void @hulk_panic(ptr @proto_panic_msg_3)
  unreachable
}

define double @Temperature.value(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %load_self = load ptr, ptr %self1, align 8
  %deg_field = getelementptr inbounds %obj.Temperature, ptr %load_self, i32 0, i32 1
  %load_deg = load double, ptr %deg_field, align 8
  ret double %load_deg
}

declare void @hulk_panic(ptr)

define ptr @main() {
entry:
  %obj_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Temperature, ptr null, i32 1) to i64))
  %vtable_slot = getelementptr inbounds %obj.Temperature, ptr %obj_alloc, i32 0, i32 0
  store ptr @vtable.Temperature, ptr %vtable_slot, align 8
  %self = alloca ptr, align 8
  store ptr %obj_alloc, ptr %self, align 8
  %deg = alloca double, align 8
  store double 3.000000e+01, ptr %deg, align 8
  %load_deg = load double, ptr %deg, align 8
  %deg_field = getelementptr inbounds %obj.Temperature, ptr %obj_alloc, i32 0, i32 1
  store double %load_deg, ptr %deg_field, align 8
  %t1 = alloca ptr, align 8
  store ptr %obj_alloc, ptr %t1, align 8
  %obj_alloc1 = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Temperature, ptr null, i32 1) to i64))
  %vtable_slot2 = getelementptr inbounds %obj.Temperature, ptr %obj_alloc1, i32 0, i32 0
  store ptr @vtable.Temperature, ptr %vtable_slot2, align 8
  %self3 = alloca ptr, align 8
  store ptr %obj_alloc1, ptr %self3, align 8
  %deg4 = alloca double, align 8
  store double 2.500000e+01, ptr %deg4, align 8
  %load_deg5 = load double, ptr %deg4, align 8
  %deg_field6 = getelementptr inbounds %obj.Temperature, ptr %obj_alloc1, i32 0, i32 1
  store double %load_deg5, ptr %deg_field6, align 8
  %t2 = alloca ptr, align 8
  store ptr %obj_alloc1, ptr %t2, align 8
  %load_t1 = load ptr, ptr %t1, align 8
  %load_t2 = load ptr, ptr %t2, align 8
  %call_max_comp = call double @max_comp(ptr %load_t1, ptr %load_t2)
  %cmptmp = fcmp oeq double %call_max_comp, 3.000000e+01
  br i1 %cmptmp, label %if_then, label %if_else

if_then:                                          ; preds = %entry
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr @str)
  br label %if_merge

if_else:                                          ; preds = %entry
  %print7 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.2, ptr @str.1)
  br label %if_merge

if_merge:                                         ; preds = %if_else, %if_then
  %iftmp_str = phi ptr [ @str, %if_then ], [ @str.1, %if_else ]
  %load_t28 = load ptr, ptr %t2, align 8
  %load_t19 = load ptr, ptr %t1, align 8
  %call_max_comp10 = call double @max_comp(ptr %load_t28, ptr %load_t19)
  %cmptmp11 = fcmp oeq double %call_max_comp10, 3.000000e+01
  br i1 %cmptmp11, label %if_then12, label %if_else13

if_then12:                                        ; preds = %if_merge
  %print15 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.4, ptr @str.3)
  br label %if_merge14

if_else13:                                        ; preds = %if_merge
  %print16 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.6, ptr @str.5)
  br label %if_merge14

if_merge14:                                       ; preds = %if_else13, %if_then12
  %iftmp_str17 = phi ptr [ @str.3, %if_then12 ], [ @str.5, %if_else13 ]
  ret ptr %iftmp_str17
}

declare ptr @malloc(i64)

declare i32 @printf(ptr, ...)
