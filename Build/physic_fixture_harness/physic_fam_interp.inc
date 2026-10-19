// Point-rotation pause and zero-offset position branches.
static void fam_interp( void )
{
	static const int32_t Rot[4][3] = {
		{ 0, 0, 0 },
		{ 16384, -8192, 7 },
		{ -32768, 65535, -1 },
		{ 1073741824, -1073741824, 123456789 }
	};
	for( int Fn = 0; Fn < 2; ++Fn )
		for( int K = 0; K < 4; ++K )
		{
			memset( g_pool_mem[0], 0, sizeof g_pool_mem[0] );
			AInterpolationPoint* P = (AInterpolationPoint*)g_pool_mem[0];
			P->Rotation = FRotator( Rot[K][0], Rot[K][1], Rot[K][2] );
			FVector Location( (FLOAT)K, (FLOAT)(K + 1), (FLOAT)(K - 2) );
			FRotator Result = Fn == 0
				? P->GetDesiredRotationAtPause( 0, Location )
				: P->GetDesiredRotationAtPosition( 0, 0, Location );
			printf( "%s %016llx %08x %08x %08x | %08x %08x %08x\n",
				Fn == 0 ? "interp_pause" : "interp_position",
				(unsigned long long)(K + 1 + Fn * 4), (uint32_t)Rot[K][0], (uint32_t)Rot[K][1], (uint32_t)Rot[K][2],
				(uint32_t)Result.Pitch, (uint32_t)Result.Yaw, (uint32_t)Result.Roll );
		}
	memset( g_pool_mem[0], 0, sizeof g_pool_mem[0] );
	memset( g_pool_mem[1], 0, sizeof g_pool_mem[1] );
	AInterpolationManager* M = (AInterpolationManager*)g_pool_mem[0];
	AActor* Owner = (AActor*)g_pool_mem[1];
	*(void**)M = g_obj_vt;
	*(void**)Owner = g_obj_vt;
	M->Owner = Owner;
	M->PhysRate = 0.f;
	Owner->Location = FVector( 7.f, -3.f, 11.f );
	Owner->Velocity = FVector( 2.f, -4.f, 8.f );
	FLOAT Dt = 0.25f;
	M->AInterpolationManager::performPhysics( Dt );
	printf( "interp_manager_idle %016llx %08x %08x %08x %08x | %08x %08x %08x %08x %08x %08x\n",
		(unsigned long long)9, fbits( Dt ), fbits( 7.f ), fbits( -3.f ), fbits( 11.f ),
		fbits( Owner->Velocity.X ), fbits( Owner->Velocity.Y ), fbits( Owner->Velocity.Z ),
		fbits( Owner->Acceleration.X ), fbits( Owner->Acceleration.Y ), fbits( Owner->Acceleration.Z ) );
	memset( g_pool_mem[0], 0, sizeof g_pool_mem[0] );
	AInterpolationPoint* P = (AInterpolationPoint*)g_pool_mem[0];
	P->bInstantNextPath = 1;
	P->Location = FVector( 5.f, -6.f, 7.f );
	FLOAT Alpha = 0.375f;
	FCoords Old( FVector( 0.f ) );
	FVector Start( 1.f, 2.f, 3.f );
	FCoords Result = P->GetInterpolatedPosition( Old, Start, Alpha, 0 );
	printf( "interp_position_instant %016llx %08x %08x %08x %08x |",
		(unsigned long long)10, fbits( Alpha ), fbits( 5.f ), fbits( -6.f ), fbits( 7.f ) );
	FLOAT Values[12] = {
		Result.Origin.X, Result.Origin.Y, Result.Origin.Z,
		Result.XAxis.X, Result.XAxis.Y, Result.XAxis.Z,
		Result.YAxis.X, Result.YAxis.Y, Result.YAxis.Z,
		Result.ZAxis.X, Result.ZAxis.Y, Result.ZAxis.Z
	};
	for( INT K = 0; K < 12; ++K )
		printf( " %08x", fbits( Values[K] ) );
	printf( "\n" );
	memset( g_pool_mem, 0, sizeof g_pool_mem );
	M = (AInterpolationManager*)g_pool_mem[0];
	Owner = (AActor*)g_pool_mem[1];
	AInterpolationPoint* Dest = (AInterpolationPoint*)g_pool_mem[2];
	g_pool[0] = (AActor*)M;
	g_pool[1] = Owner;
	g_pool[2] = (AActor*)Dest;
	*(void**)M = g_obj_vt;
	*(void**)Owner = g_obj_vt;
	*(void**)Dest = g_obj_vt;
	M->Owner = Owner;
	M->Dest = Dest;
	M->PhysRate = 0.5f;
	M->PhysAlpha = 0.f;
	M->XLevel = g_level;
	M->RemainingPause = 0.f;
	Owner->XLevel = g_level;
	Owner->bInterpolating = 1;
	Owner->bInterpolating_IgnoreRot = 1;
	Owner->Location = FVector( 3.f, -2.f, 1.f );
	Owner->Velocity = FVector( 2.f, -4.f, 8.f );
	Dest->Location = FVector( 5.f, 6.f, 7.f );
	Dest->EndControlPoint = FVector( 1.f, 0.f, 0.f );
	Dest->StartControlPoint = FVector( 1.f, 0.f, 0.f );
	Dest->PathDist = 1.f;
	g_svc = 11;
	g_loglen = 0;
	g_log[0] = 0;
	g_walk_move_calls = 0;
	FLOAT ActiveDt = 3.f;
	M->AInterpolationManager::performPhysics( ActiveDt );
	uint64_t LogHash = fnv1a( g_log, g_loglen );
	printf( "interp_manager_active %016llx %08x |",
		(unsigned long long)11, fbits( ActiveDt ) );
	printf( " %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x",
		fbits( M->PhysAlpha ), fbits( M->RemainingPause ),
		fbits( Owner->Location.X ), fbits( Owner->Location.Y ), fbits( Owner->Location.Z ),
		fbits( Owner->Velocity.X ), fbits( Owner->Velocity.Y ), fbits( Owner->Velocity.Z ),
		fbits( Owner->Acceleration.X ), fbits( Owner->Acceleration.Y ), fbits( Owner->Acceleration.Z ) );
	printf( " %08x %08x %08x\n", (uint32_t)(LogHash >> 32), (uint32_t)LogHash, (uint32_t)g_loglen );
	memset( g_pool_mem, 0, sizeof g_pool_mem );
	M = (AInterpolationManager*)g_pool_mem[0];
	Owner = (AActor*)g_pool_mem[1];
	g_pool[0] = (AActor*)M;
	g_pool[1] = Owner;
	g_pool[2] = NULL;
	*(void**)M = g_obj_vt;
	*(void**)Owner = g_obj_vt;
	M->Owner = Owner;
	M->Dest = NULL;
	M->XLevel = g_level;
	M->PhysRate = 0.5f;
	M->PhysAlpha = 0.f;
	Owner->bInterpolating = 1;
	Owner->Location = FVector( 7.f, -3.f, 11.f );
	Owner->Velocity = FVector( 2.f, -4.f, 8.f );
	g_svc = 12;
	g_loglen = 0;
	g_log[0] = 0;
	FLOAT FinishDt = 0.25f;
	M->AInterpolationManager::performPhysics( FinishDt );
	LogHash = fnv1a( g_log, g_loglen );
	printf( "interp_manager_finished %016llx %08x |",
		(unsigned long long)12, fbits( FinishDt ) );
	printf( " %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x %08x",
		fbits( M->PhysAlpha ), fbits( M->RemainingPause ),
		fbits( Owner->Location.X ), fbits( Owner->Location.Y ), fbits( Owner->Location.Z ),
		fbits( Owner->Velocity.X ), fbits( Owner->Velocity.Y ), fbits( Owner->Velocity.Z ),
		fbits( Owner->Acceleration.X ), fbits( Owner->Acceleration.Y ), fbits( Owner->Acceleration.Z ) );
	printf( " %08x %08x %08x\n", (uint32_t)(LogHash >> 32), (uint32_t)LogHash, (uint32_t)g_loglen );
}
