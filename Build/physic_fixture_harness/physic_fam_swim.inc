// Family "swim": APawn::physFlying, physSwimming, startSwimming, Swim, findWaterLine.
enum { SW_START, SW_FLY, SW_SWIM, SW_LINE, SW_PHYS, SW_COUNT };
static const char* const kSwimNames[SW_COUNT] = { "start_swimming", "phys_flying", "swim", "find_water_line", "phys_swimming" };
static const int kSwimArgW[SW_COUNT] = { 6, 2, 3, 6, 2 };

static void print_swim_case( int F, uint64_t Seed, const uint32_t* In, const uint32_t* Out )
{
	printf( "%s %016llx", kSwimNames[F], (unsigned long long)Seed );
	for( int K = 0; K < ARG0 + kSwimArgW[F]; ++K ) printf( " %08x", In[K] );
	printf( " |" );
	for( int K = 0; K < OUT_W; ++K ) printf( " %08x", Out[K] );
	printf( "\n" );
}

static void fam_swim( void )
{
	g_level->Model = (UModel*)g_swim_model_mem;
	uint64_t Master[SW_COUNT];
	for( int F = 0; F < SW_COUNT; ++F ) Master[F] = 0x6D2B79F5A4C3E281ULL ^ ((uint64_t)(F + 1) * 0x100000001B3ULL);
	uint32_t In[IN_MAX], Out[OUT_W];
	for( int F = 0; F < SW_COUNT; ++F ) for( int C = 0; C < 1200; ++C )
	{
		uint64_t Seed = sm_next( &Master[F] );
		g_gen = Seed ^ 0xA5A5A5A5A5A5A5A5ULL;
		memset( In, 0, sizeof In );
		gen_subject( In, 1u, 2u ); gen_zone( In + 32 ); gen_pool( In + POOL_OFF ); gen_hit( In + HIT_OFF );
		uint32_t* A = In + ARG0;
		if( F == SW_START ) { gen_vec(A,-500.f,500.f,5); A[3]=gen_bits(0.01f,0.2f); A[4]=gen_bits(0.f,0.2f); A[5]=gen_u(8); }
		else if( F == SW_FLY || F == SW_PHYS ) { A[0]=gen_bits(0.f,0.1f); A[1]=gen_u(8); }
		else if( F == SW_SWIM ) gen_vec(A,-100.f,100.f,0);
		else { gen_vec(A,-100.f,100.f,0); gen_vec(A+3,-100.f,100.f,0); }
		memset( g_subj_mem, 0, sizeof g_subj_mem ); memset( g_zone_mem, 0, sizeof g_zone_mem ); memset( g_pool_mem, 0, sizeof g_pool_mem );
		AActor* S = (AActor*)g_subj_mem; g_zone=(AZoneInfo*)g_zone_mem;
		for( int K=0; K<3; ++K ) g_pool[K]=(AActor*)g_pool_mem[K];
		load_zone(g_zone,In+32); g_zone->bWaterZone=(In[38]!=0);
		for( int K=0; K<3; ++K ) load_pool(g_pool[K],In+POOL_OFF+K*3);
		load_subject(S,In); S->Level=(ALevelInfo*)g_zone; S->XLevel=g_level;
		((APawn*)S)->HeadRegion=FPointRegion(g_zone,0,0);
		for( int K=0; K<3; ++K ) *(void**)g_pool[K]=g_obj_vt;
		*(void**)S=g_obj_vt; *(void**)g_zone=g_obj_vt;
		FCheckResult Hit(fval(In[HIT_OFF])); Hit.Normal=vec3(In+HIT_OFF+1); Hit.Location=vec3(In+HIT_OFF+4); Hit.Actor=pool_or_null(In[HIT_OFF+7]);
		g_svc=Seed; g_loglen=0; g_log[0]=0; uint32_t Ret=0;
		FVector V=vec3(A), End=vec3(A+3);
		g_swim_other_zone=(AZoneInfo*)g_pool[2];
		g_swim_other_zone->bWaterZone=!g_zone->bWaterZone;
		switch(F) {
		case SW_START: ((APawn*)S)->startSwimming(V,fval(A[3]),fval(A[4]),(INT)A[5]); break;
		case SW_FLY: ((APawn*)S)->physFlying(fval(A[0]),(INT)A[1]); break;
		case SW_SWIM: Ret=fbits(((APawn*)S)->Swim(V,Hit)); break;
		case SW_LINE: ((APawn*)S)->findWaterLine(V,End); break;
		case SW_PHYS: ((APawn*)S)->physSwimming(fval(A[0]),(INT)A[1]); break;
		}
		store_subject(S,Out); store_zone(g_zone,Out+32); store_hit(Hit,Out+39);
		if( F == SW_LINE ) put3( Out + 43, End );
		Out[47]=Ret;
		uint64_t H=fnv1a(g_log,g_loglen); Out[48]=(uint32_t)(H>>32); Out[49]=(uint32_t)H; Out[50]=(uint32_t)g_loglen;
		print_swim_case(F,Seed,In,Out);
	}
	fprintf(stderr,"cases %d\n",SW_COUNT*1200);
}
