// Family "move": original-product AActor::physTrailer branch records.
static char g_trailer_mesh_mem[sizeof(UMesh)] __attribute__((aligned(16)));
static void fam_move( void )
{
	uint32_t BranchMask = 0;
	for( uint64_t Case = 0; Case < 1200; ++Case )
	{
		const uint64_t Seed = 0xA0710A1000000000ULL + Case;
		uint32_t W[24] = {};
		g_gen = Seed ^ 0xA5A5A5A5A5A5A5A5ULL;
		const uint32_t Action = (uint32_t)(Case % 5);
		gen_vec( W + 0, -100.f, 100.f, 0 );
		for( int K = 0; K < 3; ++K ) W[3 + K] = gen_u( 65536 );
		gen_vec( W + 6, -100.f, 100.f, 0 );
		gen_vec( W + 9, -10.f, 10.f, 3 );
		for( int K = 0; K < 3; ++K ) W[12 + K] = gen_u( 65536 );
		gen_vec( W + 15, -8.f, 8.f, 0 );
		W[18] = gen_bits( 1.f, 50.f );
		W[19] = Action;
		gen_vec( W + 20, -20.f, 20.f, 0 );
		W[23] = 0;

		memset( g_subj_mem, 0, sizeof g_subj_mem );
		memset( g_pool_mem, 0, sizeof g_pool_mem );
		memset( g_trailer_mesh_mem, 0, sizeof g_trailer_mesh_mem );
		AActor* S = (AActor*)g_subj_mem;
		AActor* Owner = (AActor*)g_pool_mem[0];
		for( int K = 0; K < 3; ++K ) g_pool[K] = (AActor*)g_pool_mem[K];
		load_subject( S, W );
		load_subject( Owner, W );
		S->Location = vec3( W + 0 );
		S->Rotation = FRotator( (INT)W[3], (INT)W[4], (INT)W[5] );
		S->Mass = fval( W[18] );
		Owner->Location = vec3( W + 6 );
		Owner->Velocity = FVector( 0.f, 0.f, 0.f );
		Owner->Rotation = FRotator( (INT)W[12], (INT)W[13], (INT)W[14] );
		S->Owner = Action == 0 ? NULL : Owner;
		S->DrawType = Action == 1 ? DT_Sprite : DT_Mesh;
		S->AnimBone = Action == 2 ? 1 : 0;
		S->PrePivot = vec3( W + 15 );
		S->bTrailerPrePivot = Action == 1;
		S->bTrailerSameRotation = 0;
		S->bBlockActors = Action == 4;
		S->bCollideWorld = 0;
		Owner->Mesh = Action == 2 ? (UMesh*)g_trailer_mesh_mem : NULL;
		if( Action == 2 )
		{
			void** MeshVt = g_mesh_vt;
			memcpy( (void*)g_trailer_mesh_mem, &MeshVt, sizeof MeshVt );
			g_bone_coords = FCoords( vec3( W + 20 ), FVector(1,0,0), FVector(0,1,0), FVector(0,0,1) );
		}
		*(void**)S = g_obj_vt;
		*(void**)Owner = g_obj_vt;
		*(void**)g_level = g_vt;
		g_walk_mode = 0;
		g_walk_move_calls = 0;
		g_svc = Seed;
		g_loglen = 0;
		g_log[0] = 0;
		if( Action != 0 ) BranchMask |= 1u << Action;
		S->physTrailer( 0.f );
		uint32_t Out[6] = {};
		put3( Out, S->Location );
		Out[3] = (uint32_t)S->Rotation.Pitch;
		Out[4] = (uint32_t)S->Rotation.Yaw;
		Out[5] = (uint32_t)S->Rotation.Roll;
		printf( "phys_move %016llx", (unsigned long long)Seed );
		for( int K = 0; K < 24; ++K ) printf( " %08x", W[K] );
		printf( " |" );
		for( int K = 0; K < 6; ++K ) printf( " %08x", Out[K] );
		printf( " |" );
		for( size_t K = 0; K < g_loglen; ++K ) printf( "%02x", (unsigned char)g_log[K] );
		printf( "\n" );
	}
	if( BranchMask != 0x1e ) abort();
}
