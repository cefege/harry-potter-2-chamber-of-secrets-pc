// Family "smooth": original AActor::moveSmooth(Delta) driven by a scripted MoveActor plan.
// Input words: Delta 0..2, rotation 3..5, plan 6..23 (3 steps x {ret, time, normal xyz, actor}).
// Output words: ret, digest hi, digest lo, log length.
static void put_smooth_step( uint32_t* P, int Ret, float Time, const float* N, int Actor )
{
	P[0] = Ret ? 1u : 0u;
	P[1] = fbits( Time );
	P[2] = fbits( N[0] );
	P[3] = fbits( N[1] );
	P[4] = fbits( N[2] );
	P[5] = (uint32_t)Actor;
}

static void fam_smooth( void )
{
	for( uint64_t Case = 0; Case < 1200; ++Case )
	{
		const uint64_t Seed = 0x5300711E00000000ULL + Case;
		uint32_t W[24] = {};
		g_gen = Seed ^ 0x5A5A5A5A5A5A5A5AULL;
		const uint32_t Class = (uint32_t)(Case % 4);
		gen_vec( W + 0, -100.f, 100.f, 0 );
		for( int K = 0; K < 3; ++K ) W[3 + K] = gen_u( 65536 );

		const float D[3] = { fval( W[0] ), fval( W[1] ), fval( W[2] ) };
		uint32_t T[3];
		gen_vec( T, -1.f, 1.f, 0 );
		float N[3] = { fval( T[0] ), fval( T[1] ), fval( T[2] ) };
		const float Len = sqrtf( N[0] * N[0] + N[1] * N[1] + N[2] * N[2] );
		if( Len < 0.01f ) { N[0] = 1.f; N[1] = 0.f; N[2] = 0.f; }
		else { N[0] /= Len; N[1] /= Len; N[2] /= Len; }
		// Scaled along Delta: (Delta|N)^2 > |Delta|^2, so the slide dot goes negative.
		const float Steep[3] = { 4.f * D[0], 4.f * D[1], 4.f * D[2] };
		const float Zero[3] = { 0.f, 0.f, 0.f };
		const float Mid = fval( gen_bits( 0.1f, 0.9f ) );
		const float Mid2 = fval( gen_bits( 0.1f, 0.9f ) );
		const int Actor0 = gen_u( 4 ) == 3 ? -1 : (int)gen_u( 3 );
		const int Actor1 = gen_u( 4 ) == 3 ? -1 : (int)gen_u( 3 );

		uint32_t* P = W + 6;
		put_smooth_step( P + 0, 1, 1.f, Zero, -1 );
		put_smooth_step( P + 6, 1, 1.f, Zero, -1 );
		put_smooth_step( P + 12, 1, 1.f, Zero, -1 );
		if( Class == 1 )
			put_smooth_step( P + 0, 0, Mid, Steep, Actor0 );
		else if( Class == 2 )
		{
			put_smooth_step( P + 0, 0, Mid, N, Actor0 );
		}
		else if( Class == 3 )
		{
			put_smooth_step( P + 0, 0, Mid, N, Actor0 );
			put_smooth_step( P + 6, 0, Mid2, N, Actor1 );
		}

		memset( g_subj_mem, 0, sizeof g_subj_mem );
		memset( g_pool_mem, 0, sizeof g_pool_mem );
		AActor* S = (AActor*)g_subj_mem;
		for( int K = 0; K < 3; ++K ) g_pool[K] = (AActor*)g_pool_mem[K];
		uint32_t Subj[SUBJ_W + ZONE_W] = {};
		load_subject( S, Subj );
		S->Rotation = FRotator( (INT)W[3], (INT)W[4], (INT)W[5] );
		*(void**)S = g_obj_vt;
		*(void**)g_level = g_vt;
		for( int K = 0; K < 3; ++K )
		{
			const uint32_t* Q = P + 6 * K;
			g_smooth_plan[K].Ret = (int)Q[0];
			g_smooth_plan[K].Time = fval( Q[1] );
			g_smooth_plan[K].Normal = FVector( fval( Q[2] ), fval( Q[3] ), fval( Q[4] ) );
			g_smooth_plan[K].Actor = (int)Q[5];
		}
		g_walk_mode = 7;
		g_walk_move_calls = 0;
		g_svc = Seed;
		g_loglen = 0;
		g_log[0] = 0;

		const int Ret = S->moveSmooth( vec3( W + 0 ) );
		const uint64_t H = fnv1a( g_log, g_loglen );

		printf( "phys_smooth %016llx", (unsigned long long)Seed );
		for( int K = 0; K < 24; ++K ) printf( " %08x", W[K] );
		printf( " |" );
		printf( " %08x %08x %08x %08x", (uint32_t)Ret, (uint32_t)(H >> 32), (uint32_t)H, (uint32_t)g_loglen );
		printf( "\n" );
	}
	g_walk_mode = 0;
}
