; ModuleID = 'tests'
source_filename = "tests"

%vtable.Bike = type { i64, ptr, ptr }
%vtable.Car = type { i64, ptr, ptr }
%vtable.Vehicle = type { i64, ptr, ptr }
%obj.Vehicle = type { ptr, double }
%obj.Car = type { ptr, double, double }
%obj.Bike = type { ptr, double }

@vtable.Bike = constant %vtable.Bike { i64 0, ptr @Bike.move, ptr @Vehicle.max_speed }
@vtable.Car = constant %vtable.Car { i64 1, ptr @Car.move, ptr @Vehicle.max_speed }
@vtable.Vehicle = constant %vtable.Vehicle { i64 2, ptr @Vehicle.move, ptr @Vehicle.max_speed }
@str = private unnamed_addr constant [7 x i8] c"moving\00", align 1
@str.1 = private unnamed_addr constant [8 x i8] c"driving\00", align 1
@str.2 = private unnamed_addr constant [8 x i8] c"cycling\00", align 1
@str.3 = private unnamed_addr constant [8 x i8] c"driving\00", align 1
@str.4 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.5 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.6 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.7 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.8 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.9 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.10 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.11 = private unnamed_addr constant [8 x i8] c"cycling\00", align 1
@str.12 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.13 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.14 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.15 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.16 = private unnamed_addr constant [3 x i8] c"ok\00", align 1
@print_fmt_str.17 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1
@str.18 = private unnamed_addr constant [5 x i8] c"fail\00", align 1
@print_fmt_str.19 = private unnamed_addr constant [4 x i8] c"%s\0A\00", align 1

define ptr @Vehicle.move(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  ret ptr @str
}

define double @Vehicle.max_speed(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  %load_self = load ptr, ptr %self1, align 8
  %speed_field = getelementptr inbounds %obj.Vehicle, ptr %load_self, i32 0, i32 1
  %load_speed = load double, ptr %speed_field, align 8
  ret double %load_speed
}

define ptr @Car.move(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  ret ptr @str.1
}

define ptr @Bike.move(ptr %self) {
entry:
  %self1 = alloca ptr, align 8
  store ptr %self, ptr %self1, align 8
  ret ptr @str.2
}

define ptr @main() {
entry:
  %obj_alloc = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Car, ptr null, i32 1) to i64))
  %vtable_slot = getelementptr inbounds %obj.Car, ptr %obj_alloc, i32 0, i32 0
  store ptr @vtable.Car, ptr %vtable_slot, align 8
  %self = alloca ptr, align 8
  store ptr %obj_alloc, ptr %self, align 8
  %spd = alloca double, align 8
  store double 1.200000e+02, ptr %spd, align 8
  %d = alloca double, align 8
  store double 4.000000e+00, ptr %d, align 8
  %load_spd = load double, ptr %spd, align 8
  %spd1 = alloca double, align 8
  store double %load_spd, ptr %spd1, align 8
  %load_spd2 = load double, ptr %spd1, align 8
  %speed_field = getelementptr inbounds %obj.Car, ptr %obj_alloc, i32 0, i32 1
  store double %load_spd2, ptr %speed_field, align 8
  %load_d = load double, ptr %d, align 8
  %doors_field = getelementptr inbounds %obj.Car, ptr %obj_alloc, i32 0, i32 2
  store double %load_d, ptr %doors_field, align 8
  %v = alloca ptr, align 8
  store ptr %obj_alloc, ptr %v, align 8
  %load_v = load ptr, ptr %v, align 8
  %vtable_ptr = getelementptr inbounds %obj.Vehicle, ptr %load_v, i32 0, i32 0
  %vtable_load = load ptr, ptr %vtable_ptr, align 8
  %method_ptr = getelementptr inbounds %vtable.Vehicle, ptr %vtable_load, i32 0, i32 1
  %method_load = load ptr, ptr %method_ptr, align 8
  %call_Vehicle_move = call ptr %method_load(ptr %load_v)
  %streqtmp = call i32 @strcmp(ptr %call_Vehicle_move, ptr @str.3)
  %strcmptmp = icmp eq i32 %streqtmp, 0
  br i1 %strcmptmp, label %if_then, label %if_else

if_then:                                          ; preds = %entry
  %print = call i32 (ptr, ...) @printf(ptr @print_fmt_str, ptr @str.4)
  br label %if_merge

if_else:                                          ; preds = %entry
  %print3 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.6, ptr @str.5)
  br label %if_merge

if_merge:                                         ; preds = %if_else, %if_then
  %iftmp_str = phi ptr [ @str.4, %if_then ], [ @str.5, %if_else ]
  %load_v4 = load ptr, ptr %v, align 8
  %vtable_ptr5 = getelementptr inbounds %obj.Vehicle, ptr %load_v4, i32 0, i32 0
  %vtable_load6 = load ptr, ptr %vtable_ptr5, align 8
  %method_ptr7 = getelementptr inbounds %vtable.Vehicle, ptr %vtable_load6, i32 0, i32 2
  %method_load8 = load ptr, ptr %method_ptr7, align 8
  %call_Vehicle_max_speed = call double %method_load8(ptr %load_v4)
  %cmptmp = fcmp oeq double %call_Vehicle_max_speed, 1.200000e+02
  br i1 %cmptmp, label %if_then9, label %if_else10

if_then9:                                         ; preds = %if_merge
  %print12 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.8, ptr @str.7)
  br label %if_merge11

if_else10:                                        ; preds = %if_merge
  %print13 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.10, ptr @str.9)
  br label %if_merge11

if_merge11:                                       ; preds = %if_else10, %if_then9
  %iftmp_str14 = phi ptr [ @str.7, %if_then9 ], [ @str.9, %if_else10 ]
  %obj_alloc15 = call ptr @malloc(i64 ptrtoint (ptr getelementptr (%obj.Bike, ptr null, i32 1) to i64))
  %vtable_slot16 = getelementptr inbounds %obj.Bike, ptr %obj_alloc15, i32 0, i32 0
  store ptr @vtable.Bike, ptr %vtable_slot16, align 8
  %self17 = alloca ptr, align 8
  store ptr %obj_alloc15, ptr %self17, align 8
  %spd18 = alloca double, align 8
  store double 2.500000e+01, ptr %spd18, align 8
  %load_spd19 = load double, ptr %spd18, align 8
  %spd20 = alloca double, align 8
  store double %load_spd19, ptr %spd20, align 8
  %load_spd21 = load double, ptr %spd20, align 8
  %speed_field22 = getelementptr inbounds %obj.Bike, ptr %obj_alloc15, i32 0, i32 1
  store double %load_spd21, ptr %speed_field22, align 8
  %b = alloca ptr, align 8
  store ptr %obj_alloc15, ptr %b, align 8
  %load_b = load ptr, ptr %b, align 8
  %vtable_ptr23 = getelementptr inbounds %obj.Vehicle, ptr %load_b, i32 0, i32 0
  %vtable_load24 = load ptr, ptr %vtable_ptr23, align 8
  %method_ptr25 = getelementptr inbounds %vtable.Vehicle, ptr %vtable_load24, i32 0, i32 1
  %method_load26 = load ptr, ptr %method_ptr25, align 8
  %call_Vehicle_move27 = call ptr %method_load26(ptr %load_b)
  %streqtmp28 = call i32 @strcmp(ptr %call_Vehicle_move27, ptr @str.11)
  %strcmptmp29 = icmp eq i32 %streqtmp28, 0
  br i1 %strcmptmp29, label %if_then30, label %if_else31

if_then30:                                        ; preds = %if_merge11
  %print33 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.13, ptr @str.12)
  br label %if_merge32

if_else31:                                        ; preds = %if_merge11
  %print34 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.15, ptr @str.14)
  br label %if_merge32

if_merge32:                                       ; preds = %if_else31, %if_then30
  %iftmp_str35 = phi ptr [ @str.12, %if_then30 ], [ @str.14, %if_else31 ]
  %load_b36 = load ptr, ptr %b, align 8
  %vtable_ptr37 = getelementptr inbounds %obj.Vehicle, ptr %load_b36, i32 0, i32 0
  %vtable_load38 = load ptr, ptr %vtable_ptr37, align 8
  %method_ptr39 = getelementptr inbounds %vtable.Vehicle, ptr %vtable_load38, i32 0, i32 2
  %method_load40 = load ptr, ptr %method_ptr39, align 8
  %call_Vehicle_max_speed41 = call double %method_load40(ptr %load_b36)
  %cmptmp42 = fcmp oeq double %call_Vehicle_max_speed41, 2.500000e+01
  br i1 %cmptmp42, label %if_then43, label %if_else44

if_then43:                                        ; preds = %if_merge32
  %print46 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.17, ptr @str.16)
  br label %if_merge45

if_else44:                                        ; preds = %if_merge32
  %print47 = call i32 (ptr, ...) @printf(ptr @print_fmt_str.19, ptr @str.18)
  br label %if_merge45

if_merge45:                                       ; preds = %if_else44, %if_then43
  %iftmp_str48 = phi ptr [ @str.16, %if_then43 ], [ @str.18, %if_else44 ]
  ret ptr %iftmp_str48
}

declare ptr @malloc(i64)

declare i32 @strcmp(ptr, ptr)

declare i32 @printf(ptr, ...)
