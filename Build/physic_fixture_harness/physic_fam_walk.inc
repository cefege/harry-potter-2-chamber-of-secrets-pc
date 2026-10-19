// Family "walk": iteration limit, slope movement, auto-jump prediction, ledge
// avoidance, step-up, swimming transition, and may-fall event.
static void fam_walk( void )
{
	for( uint64_t Case = 0; Case < 1200; ++Case )
	{
		uint32_t W[59] = {};
		uint32_t Out[OUT_W] = {};
		const uint64_t Seed = 0xA11C000000000000ULL + Case;
		const uint32_t Mode = (uint32_t)(Case % 7);
		g_gen = Seed ^ 0xA5A5A5A5A5A5A5A5ULL;
		gen_subject( W, 1, 1 );
		gen_zone( W + 32 );
		gen_pool( W + POOL_OFF );
		gen_hit( W + HIT_OFF );
		W[31] = Mode == 2 ? 2 : 1;
		W[6] = W[7] = W[8] = 0;
		W[9] = fbits( 1.f );
		W[10] = W[11] = 0;
		W[12] = 1; // PHYS_Walking.
		W[13] = 1;
		W[21] = fbits( 1.f ); // desired speed
		W[22] = fbits( 1.f ); // floor distance stays within 2.4 units
		W[23] = Mode == 4 ? 0u : fbits( 1.f );
		W[25] = fbits( 20.f );
		W[26] = fbits( 40.f );
		W[32] = W[33] = W[35] = W[36] = W[37] = 0;
		W[34] = fbits( -950.f );
		W[48] = fbits( 1.f );
		W[49] = W[50] = 0;
		W[51] = fbits( 1.f );
		W[55] = 0xffffffffu;
		W[56] = fbits( 0.01f );
		W[57] = Mode ? 7 : 8;
		W[58] = Mode;

		memset( g_subj_mem, 0, sizeof g_subj_mem );
		memset( g_zone_mem, 0, sizeof g_zone_mem );
		memset( g_pool_mem, 0, sizeof g_pool_mem );
		AActor* S = (AActor*)g_subj_mem;
		g_zone = (AZoneInfo*)g_zone_mem;
		for( int K = 0; K < 3; ++K ) g_pool[K] = (AActor*)g_pool_mem[K];
		load_zone( g_zone, W + 32 );
		for( int K = 0; K < 3; ++K ) load_pool( g_pool[K], W + POOL_OFF + K * 3 );
		load_subject( S, W );
		S->Region.ZoneNumber = 1;
		g_zone->ZoneGroundFriction = 0.f;
		((APawn*)S)->GroundSpeed = 100.f;
		((APawn*)S)->bCanFly = Mode == 1 || Mode == 2 || Mode == 4 || Mode == 5;
		((APawn*)S)->bAvoidLedges = Mode == 3;
		((APawn*)S)->bCanJump = Mode == 6;
		if( Mode == 2 ) ((APlayerPawn*)S)->bAutoJump = 1;
		((APawn*)S)->JumpZ = 0.f;
		g_walk_mode = (int)Mode;
		g_walk_move_calls = 0;
		g_walk_slc_calls = 0;
		for( int K = 0; K < 3; ++K ) *(void**)g_pool[K] = g_obj_vt;
		*(void**)S = g_obj_vt;
		*(void**)g_zone = g_obj_vt;

		FCheckResult Hit( fval( W[HIT_OFF] ) );
		Hit.Normal = vec3( W + HIT_OFF + 1 );
		Hit.Location = vec3( W + HIT_OFF + 4 );
		Hit.Actor = pool_or_null( W[HIT_OFF + 7] );
		g_svc = Seed;
		g_loglen = 0;
		g_log[0] = 0;
		((APawn*)S)->physWalking( fval( W[56] ), (INT)W[57] );

		store_subject( S, Out );
		store_zone( g_zone, Out + 32 );
		store_hit( Hit, Out + 39 );
		const uint64_t LogHash = fnv1a( g_log, g_loglen );
		Out[48] = (uint32_t)(LogHash >> 32);
		Out[49] = (uint32_t)LogHash;
		Out[50] = (uint32_t)g_loglen;
		printf( "phys_walking %016llx", (unsigned long long)Seed );
		for( int K = 0; K < 59; ++K ) printf( " %08x", W[K] );
		printf( " |" );
		for( int K = 0; K < OUT_W; ++K ) printf( " %08x", Out[K] );
		printf( "\n" );
	}
}
