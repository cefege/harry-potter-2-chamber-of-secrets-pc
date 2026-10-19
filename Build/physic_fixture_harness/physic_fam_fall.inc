// Family "fall": AActor::physFalling. Input words retain the common subject,
// zone, pool and hit layout, followed by deltaTime/Iterations and fall-only
// ZoneNumber, Role, AirControl, Zone bWaterZone, fluid friction, terminal
// velocity, FootRegion bWaterZone, and bBounce. Output appends fall properties.
static char g_fall_mover_class_raw[sizeof(UClass)] __attribute__((aligned(16)));
static char g_fall_inventory_class_raw[sizeof(UClass)] __attribute__((aligned(16)));
UClass* AInventory::PrivateStaticClass = (UClass*)g_fall_inventory_class_raw;
static uint32_t fall_log_mask( void )
{
	uint32_t M = 0;
	if( strstr( g_log, "slc " ) ) M |= 1u;
	if( strstr( g_log, "move " ) ) M |= 2u;
	if( strstr( g_log, "event_hit_wall " ) ) M |= 4u;
	if( strstr( g_log, "event_landed " ) ) M |= 8u;
	if( strstr( g_log, "event_mount " ) ) M |= 16u;
	if( strstr( g_log, "event_fell_out_of_world\n" ) ) M |= 64u;
	return M;
}

static void fam_fall( void )
{
	for( uint64_t Case = 0; Case < 1200; ++Case )
	{
		const uint64_t Seed = 0xFA11000000000000ULL + Case;
		uint32_t W[66] = {};
		uint32_t Out[56] = {};
		g_gen = Seed ^ 0xA5A5A5A5A5A5A5A5ULL;
		gen_subject( W, 1, 4 );
		gen_zone( W + 32 );
		gen_pool( W + POOL_OFF );
		gen_hit( W + HIT_OFF );
		W[13] = W[31] == 1 || W[31] == 2;
		W[56] = gen_bits( 0.001f, 0.2f );
		W[57] = gen_u( 4 );
		W[58] = gen_u( 20 ) ? 1u : 0u;
		W[59] = (gen_u( 5 ) == 0) ? 4u : gen_u( 5 );
		W[60] = gen_bits( 0.f, 0.3f );
		W[61] = gen_u( 2 );
		W[62] = gen_bits( 0.f, 1.f );
		W[64] = gen_u( 2 );
		W[63] = gen_bits( 100.f, 1200.f );
		W[12] = 2; // Keep cases in PHYS_Falling, not startSwimming.
		W[65] = gen_u( 2 );

		memset( g_subj_mem, 0, sizeof g_subj_mem );
		memset( g_zone_mem, 0, sizeof g_zone_mem );
		memset( g_pool_mem, 0, sizeof g_pool_mem );
		char FootMem[8192] __attribute__((aligned(16)));
		memset( FootMem, 0, sizeof FootMem );
		AZoneInfo* FootZone = (AZoneInfo*)FootMem;
		memset( g_fall_mover_class_raw, 0, sizeof g_fall_mover_class_raw );
		memset( g_fall_inventory_class_raw, 0, sizeof g_fall_inventory_class_raw );
		((UClass*)g_fall_mover_class_raw)->SuperField = g_cls[0];
		((UClass*)g_fall_inventory_class_raw)->SuperField = g_cls[0];
		AActor* S = (AActor*)g_subj_mem;
		g_zone = (AZoneInfo*)g_zone_mem;
		for( int K = 0; K < 3; ++K ) g_pool[K] = (AActor*)g_pool_mem[K];
		load_zone( g_zone, W + 32 );
		g_zone->bWaterZone = W[61] != 0;
		g_zone->ZoneFluidFriction = fval( W[62] );
		g_zone->ZoneTerminalVelocity = fval( W[63] );
		for( int K = 0; K < 3; ++K ) load_pool( g_pool[K], W + POOL_OFF + K * 3 );
		load_subject( S, W );
		S->Region.ZoneNumber = W[58];
		S->Role = (BYTE)W[59];
		((APawn*)S)->AirControl = fval( W[60] );
		FootZone->bWaterZone = W[64] != 0;
		FootZone->ZoneFluidFriction = fval( W[62] );
		((APawn*)S)->FootRegion.Zone = FootZone;
		S->bBounce = W[65] != 0;
		for( int K = 0; K < 3; ++K ) *(void**)g_pool[K] = g_obj_vt;
		*(void**)S = g_obj_vt;
		*(void**)g_zone = g_obj_vt;
		*(void**)FootZone = g_obj_vt;

		FCheckResult Hit( 1.f );
		g_svc = Seed;
		g_loglen = 0;
		g_log[0] = 0;
		((AActor*)S)->physFalling( fval( W[56] ), (INT)W[57] );

		store_subject( S, Out );
		store_zone( g_zone, Out + 32 );
		store_hit( Hit, Out + 39 );
		const uint64_t LogHash = fnv1a( g_log, g_loglen );
		Out[48] = (uint32_t)(LogHash >> 32);
		Out[49] = (uint32_t)LogHash;
		Out[50] = (uint32_t)g_loglen;
		Out[51] = W[58];
		Out[47] = fall_log_mask();
		if( W[65] && strstr( g_log, "event_hit_wall " ) ) Out[47] |= 32u;
		Out[52] = W[59];
		Out[53] = W[61];
		Out[54] = fbits( g_zone->ZoneFluidFriction );
		Out[55] = fbits( g_zone->ZoneTerminalVelocity );
		printf( "phys_falling %016llx", (unsigned long long)Seed );
		for( int K = 0; K < 66; ++K ) printf( " %08x", W[K] );
		printf( " |" );
		for( int K = 0; K < 56; ++K ) printf( " %08x", Out[K] );
		printf( "\n" );
	}
}
