// Family "dispatch": FindBase, setPhysics, performPhysics and physicsRotation.
static void fam_dispatch( void )
{
	for( uint64_t Case = 0; Case < 1200; ++Case )
	{
		const uint64_t Seed = 0xD15A7C0000000000ULL + Case;
		uint32_t W[67] = {};
		uint32_t Out[57] = {};
		g_gen = Seed ^ 0xA5A5A5A5A5A5A5A5ULL;
		const uint32_t Action = Case % 6;
		const bool PawnCase = Action == 3 || Action == 5;
		gen_subject( W, PawnCase ? 1u : 0u, PawnCase ? 2u : 1u );
		gen_zone( W + 32 );
		gen_pool( W + POOL_OFF );
		W[56] = gen_bits( 0.001f, 0.2f );
		W[57] = gen_u( 12 );
		W[58] = Action;
		W[59] = gen_bits( 5.f, 60.f );
		W[60] = gen_bits( 10.f, 80.f );
		W[61] = W[6]; W[62] = W[7]; W[63] = W[8];
		W[64] = gen_bits( -50.f, 50.f );
		W[65] = gen_bits( -1.f, 1.f );
		W[66] = gen_u( 2 );

		memset( g_subj_mem, 0, sizeof g_subj_mem );
		memset( g_zone_mem, 0, sizeof g_zone_mem );
		memset( g_pool_mem, 0, sizeof g_pool_mem );
		AActor* S = (AActor*)g_subj_mem;
		APawn* P = (APawn*)S;
		g_zone = (AZoneInfo*)g_zone_mem;
		for( int K = 0; K < 3; ++K ) g_pool[K] = (AActor*)g_pool_mem[K];
		load_zone( g_zone, W + 32 );
		for( int K = 0; K < 3; ++K ) load_pool( g_pool[K], W + POOL_OFF + K * 3 );
		load_subject( S, W );
		S->OldLocation = vec3( W + 17 );
		P->StepHeight = fval( W[59] );
		P->StepWidth = fval( W[60] );
		P->AvgPhysicsTime = fval( W[64] );
		P->MoveTimer = fval( W[65] );
		S->RotationRate = FRotator( (INT)W[3] % 1200, (INT)W[4] % 2400, (INT)W[5] % 800 );
		S->DesiredRotation = FRotator( (INT)W[3] + 500, (INT)W[4] - 700, (INT)W[5] + 300 );
		S->bRotateToDesired = W[66] != 0;
		S->bFixedRotationDir = W[57] & 1;
		for( int K = 0; K < 3; ++K ) *(void**)g_pool[K] = g_obj_vt;
		*(void**)S = g_obj_vt;
		*(void**)g_zone = g_obj_vt;

		FCheckResult Hit( 1.f );
		g_svc = Seed;
		g_loglen = 0;
		g_log[0] = 0;
		switch( Action )
		{
		case 0: S->FindBase(); break;
		case 1: S->setPhysics( (BYTE)(W[57] % 12), (W[66] ? g_pool[0] : NULL) ); break;
		case 2: S->Physics = PHYS_Interpolating; S->performPhysics( fval( W[56] ) ); break;
		case 3: S->Physics = PHYS_Interpolating; P->performPhysics( fval( W[56] ) ); break;
		case 4: S->physicsRotation( fval( W[56] ) ); break;
		case 5: P->physicsRotation( fval( W[56] ), vec3( W + 61 ) ); break;
		}
		store_subject( S, Out );
		store_zone( g_zone, Out + 32 );
		store_hit( Hit, Out + 39 );
		Out[47] = fbits( P->StepHeight );
		Out[48] = fbits( P->StepWidth );
		Out[49] = fbits( S->SavedPrePivotZ );
		Out[50] = (uint32_t)(fnv1a( g_log, g_loglen ) >> 32);
		Out[51] = (uint32_t)fnv1a( g_log, g_loglen );
		Out[52] = (uint32_t)g_loglen;
		Out[53] = Action;
		Out[54] = fbits( P->AvgPhysicsTime );
		Out[55] = fbits( P->MoveTimer );
		Out[56] = (uint32_t)S->bFixedRotationDir;
		printf( "phys_dispatch %016llx", (unsigned long long)Seed );
		for( int K = 0; K < 67; ++K ) printf( " %08x", W[K] );
		printf( " |" );
		for( int K = 0; K < 57; ++K ) printf( " %08x", Out[K] );
		printf( "\n" );
	}
}
