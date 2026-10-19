/*=============================================================================
	Hp2LevelServices.h: Level services called by the Rust level port
	(crates/hp2-engine/src/level.rs, level_persist.rs, level_net.rs).
	Each group's struct is #[repr(C)] in Rust and reaches C++ through hp2_rs.h.
	Each group's table is defined in Hp2LevelServices.cpp.
=============================================================================*/

#ifndef HP2LEVELSERVICES_H
#define HP2LEVELSERVICES_H

#include "hp2_rs.h"

extern const Hp2LevelCoreServices GHp2LevelCoreServices;
extern const Hp2LevelPersistServices GHp2LevelPersistServices;
extern const Hp2LevelNetServices GHp2LevelNetServices;

#endif
