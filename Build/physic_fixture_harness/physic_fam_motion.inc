// Family "motion": projectile, rolling, spider, checkFloor, and findNewFloor.
static char g_motion_projectile_class_raw[sizeof(UClass)] __attribute__((aligned(16)));
static char g_motion_mover_class_raw[sizeof(UClass)] __attribute__((aligned(16)));
UClass* AProjectile::PrivateStaticClass = (UClass*)g_motion_projectile_class_raw;
UClass* AMover::PrivateStaticClass = (UClass*)g_motion_mover_class_raw;

static uint32_t motion_log_mask( void )
{
	uint32_t M = 0;
	if( strstr( g_log, "move " ) ) M |= 1u;
	if( strstr( g_log, "slc " ) ) M |= 2u;
	if( strstr( g_log, "event_hit_wall " ) ) M |= 4u;
	if( strstr( g_log, "event_falling" ) ) M |= 8u;
	if( strstr( g_log, "set_base " ) ) M |= 16u;
	if( strstr( g_log, "slc_source -1 1\n" ) ) M |= 32u;
	return M;
}

static void fam_motion( void )
{
	for( uint64_t Case = 0; Case < 1200; ++Case )
	{
		const uint64_t Seed = 0xA0710A0000000000ULL + Case;
		uint32_t W[67] = {};
		uint32_t Out[57] = {};
		g_gen = Seed ^ 0xA5A5A5A5A5A5A5A5ULL;
		const uint32_t Action = Case % 5;
		gen_subject( W, 1, 1 );
		gen_zone( W + 32 );
		gen_pool( W + POOL_OFF );
		W[56] = gen_bits( 0.001f, 0.2f );
		W[57] = gen_u( 4 );
		W[58] = Action;
		W[59] = 1;
		W[60] = gen_bits( 80.f, 1200.f );
		gen_vec( W + 61, -1.f, 1.f, 5 );
		W[64] = gen_u( 2 );
		W[65] = gen_bits( 0.5f, 2.5f );
		W[66] = gen_bits( 0.f, 0.5f );

		memset( g_subj_mem, 0, sizeof g_subj_mem );
		memset( g_zone_mem, 0, sizeof g_zone_mem );
		memset( g_pool_mem, 0, sizeof g_pool_mem );
		memset( g_motion_projectile_class_raw, 0, sizeof g_motion_projectile_class_raw );
		memset( g_motion_mover_class_raw, 0, sizeof g_motion_mover_class_raw );
		((UClass*)g_motion_projectile_class_raw)->SuperField = g_cls[0];
		((UClass*)g_motion_mover_class_raw)->SuperField = g_cls[0];
		AActor* S = (AActor*)g_subj_mem;
		g_zone = (AZoneInfo*)g_zone_mem;
		for( int K = 0; K < 3; ++K ) g_pool[K] = (AActor*)g_pool_mem[K];
		load_zone( g_zone, W + 32 );
		g_zone->ZoneGroundFriction = fval( W[65] );
		g_zone->ZoneFluidFriction = fval( W[66] );
		for( int K = 0; K < 3; ++K ) load_pool( g_pool[K], W + POOL_OFF + K * 3 );
		if( W[64] ) g_pool[0]->Class = AMover::StaticClass();
		load_subject( S, W );
		S->Region.ZoneNumber = W[59];
		S->bBounce = W[64] != 0;
		((APawn*)S)->Floor = vec3( W + 61 );
		if( Action == 0 ) { S->Class = AProjectile::StaticClass(); S->Physics = PHYS_Projectile; ((AProjectile*)S)->MaxSpeed = fval( W[60] ); }
		else if( Action == 1 ) { S->Class = g_cls[0]; S->Physics = PHYS_Rolling; }
		else { S->Class = APawn::StaticClass(); S->Physics = PHYS_Spider; }
		for( int K = 0; K < 3; ++K ) *(void**)g_pool[K] = g_obj_vt;
		*(void**)S = g_obj_vt;
		*(void**)g_zone = g_obj_vt;

		FCheckResult Hit( 1.f );
		g_svc = Seed;
		g_loglen = 0;
		g_log[0] = 0;
		int Ret = 0;
		switch( Action )
		{
		case 0: S->physProjectile( fval( W[56] ), (INT)W[57] ); break;
		case 1: S->physRolling( fval( W[56] ), (INT)W[57] ); break;
		case 2: ((APawn*)S)->physSpider( fval( W[56] ), (INT)W[57] ); break;
		case 3: Ret = ((APawn*)S)->checkFloor( vec3( W + 61 ), Hit ); break;
		case 4: Ret = ((APawn*)S)->findNewFloor( S->Location, fval( W[56] ), fval( W[56] ), (INT)W[57] ); break;
		}
		store_subject( S, Out );
		store_zone( g_zone, Out + 32 );
		store_hit( Hit, Out + 39 );
		put3( Out + 47, ((APawn*)S)->Floor );
		Out[50] = (uint32_t)(fnv1a( g_log, g_loglen ) >> 32);
		Out[51] = (uint32_t)fnv1a( g_log, g_loglen );
		Out[52] = (uint32_t)g_loglen;
		Out[53] = Action;
		Out[54] = (uint32_t)Ret;
		Out[55] = W[60];
		Out[56] = motion_log_mask();
		printf( "phys_motion %016llx", (unsigned long long)Seed );
		for( int K = 0; K < 67; ++K ) printf( " %08x", W[K] );
		printf( " |" );
		for( int K = 0; K < 57; ++K ) printf( " %08x", Out[K] );
		printf( "\n" );
	}
}
