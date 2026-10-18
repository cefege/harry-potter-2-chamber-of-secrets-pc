/*=============================================================================

    UnFractal.cpp: Unreal's  Animated Realtime Texture  engine  / main file
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

    Revision history:
        * Created by Erik de Neve 1996,97
        * Major revised version; slow-fire and techbumps - July 97
		* Tim merged with Unreal object classes.   Aug 97
		* Erik - ongoing revisions & incorporating requests Sept 97 - Dec 97
		* Erik - Jan4 - switched off palette copying - caused problems since
		                non-unique palette names were generated for the 
						duplicates - clashed on merging different UTX-es.
		* Tim/Erik - Sept-Oct  98
		  - Adapted for on-demand loading using GetInfo calls before all 
		    referred-texture bitmap accesses (wet/ice textures);
		  - Eliminated self-referencing textures recursive GetInfo() disasters.
		  - Rescaling logic for wet/ice textures revised.

		* Future revision remarks
		  - Make sure all non-serialized vars are initialized in the constructors,
		    and all serialized vars are set one-time only in the ::Init functions.
		  - PostLoad gets called in the editor at edit-time, after properties are 
		    edited ( eg. sliders moved ) etc.
		  - For palette changes to become visible while editing you might
		    need to use GCache.Flush() -> drastic but works.
		  - Palettes and sparks are implemented as dynamic arrays. 
		    See  Core: UnTemplate.h
		  - The Destroy() function is where you want memory deallocation to 
		    take place, never use the actual "~destructor()" in Unreal -> 
			because of the way garbage-collection is implemented.
          - All new textures need some (dummy) palette at ::Init time in all 
		    cases, the engine chokes on them otherwise.
		  - If extra memory needs to be allocated for an effect, we do it by 
		    checking the pointer for null and allocating inside PostLoad, not 
			in the ctor because that's too early, some needed things like texture 
			dimensions aren't set yet at ctor time.
          - Better intelligent in-editor awareness possible. ( UBOOL GIsEditor;)
 
=============================================================================*/

#include "UnFractal.h"
#include "HP2TraceHooks.h"
#include "hp2_rs.h"

// Trace hook table handed to every hp2-fire entry. Shared by all fire slices.
// Trampoline parameters use the cbindgen types so they match the FireTraceHooks fields.
static void FireInitTablesTrampoline( uint64_t Before, uint64_t After, uint64_t Fingerprint )
{
	HP2FireTextureTraceInitTables( Before, After, Fingerprint );
}

static void FireSpeedRandTrampoline( uint32_t StateIndex, uint8_t Value )
{
	HP2FireTextureTraceSpeedRand( StateIndex, Value );
}

static void FireBurnWriteTrampoline( void* Texture, int32_t SparkIndex, uint32_t Offset, uint8_t Before, uint8_t Value )
{
	HP2FireTextureTraceBurnWrite( (UFireTexture*)Texture, SparkIndex, Offset, Before, Value );
}

static FireTraceHooks FireTraceHooksForRust()
{
	FireTraceHooks Hooks;
	Hooks.init_tables = FireInitTablesTrampoline;
	Hooks.speed_rand = FireSpeedRandTrampoline;
	Hooks.burn_write = FireBurnWriteTrampoline;
	return Hooks;
}


// Copy palettes rather than refer to them when colorizing Fire and Wave textures.
#define COPYPALETTE 1 

/*----------------------------------------------------------------------------
	Globals.
----------------------------------------------------------------------------*/

// Macros.
#define WIDTHBYTES(bits) ((((bits) + 31) / 32) * 4)

// Constants.
#define STAR_CUTOFF		38		/*  Cloud thickness above which stars vanish    */
#define DEL_RANGE		12		/*  Mouse range for deleting sparks/drops etc   */
#define LOWESTGLOW		50		/*  Delete glowing-out sparks below this intensity  */
#define LIGHTPHASEBIAS	32		/*	bias for lightphasetable  */

// Globals.
extern "C"
{
	BYTE	PhaseTable		 [256];
	BYTE    SignedPhaseTable [256];
	BYTE    LightPhaseTable  [256];
}


struct FWaterHostState
{
	BYTE* SourceFields;
	BYTE* LocalSourceBitmap;

	FWaterHostState()
	: SourceFields(NULL)
	, LocalSourceBitmap(NULL)
	{}
};

static TMap<UWaterTexture*,FWaterHostState>* GWaterHostStates = NULL;

static FWaterHostState* FindWaterHostState( const UWaterTexture* Owner )
{
	return GWaterHostStates
		? GWaterHostStates->Find(const_cast<UWaterTexture*>(Owner))
		: NULL;
}

static FWaterHostState& GetWaterHostState( UWaterTexture* Owner )
{
	check(Owner);
	if( !GWaterHostStates )
		GWaterHostStates = new TMap<UWaterTexture*,FWaterHostState>;
	if( FWaterHostState* State=GWaterHostStates->Find(Owner) )
		return *State;
	return GWaterHostStates->Set(Owner, FWaterHostState());
}

static void RemoveWaterHostStateIfEmpty( UWaterTexture* Owner, FWaterHostState* State )
{
	if( State && !State->SourceFields && !State->LocalSourceBitmap )
	{
		check(GWaterHostStates);
		check(GWaterHostStates->Remove(Owner) == 1);
		if( GWaterHostStates->Num() == 0 )
		{
			delete GWaterHostStates;
			GWaterHostStates = NULL;
		}
	}
}

static BYTE* FindWaterSourceFields( const UWaterTexture* Owner )
{
	FWaterHostState* State = FindWaterHostState(Owner);
	return State ? State->SourceFields : NULL;
}

static BYTE* AllocateWaterSourceFields( UWaterTexture* Owner, INT Size )
{
	check(Size > 0);
	FWaterHostState& State = GetWaterHostState(Owner);
	check(!State.SourceFields);
	State.SourceFields = new BYTE[Size];
	return State.SourceFields;
}

static void FreeWaterSourceFields( UWaterTexture* Owner )
{
	FWaterHostState* State = FindWaterHostState(Owner);
	if( !State )
		return;
	delete[] State->SourceFields;
	State->SourceFields = NULL;
	RemoveWaterHostStateIfEmpty(Owner, State);
}

static BYTE* FindWetLocalSourceBitmap( const UWetTexture* Owner )
{
	FWaterHostState* State = FindWaterHostState(Owner);
	return State ? State->LocalSourceBitmap : NULL;
}

static BYTE* AllocateWetLocalSourceBitmap( UWetTexture* Owner, INT Size )
{
	check(Size > 0);
	FWaterHostState& State = GetWaterHostState(Owner);
	check(!State.LocalSourceBitmap);
	State.LocalSourceBitmap = new BYTE[Size];
	return State.LocalSourceBitmap;
}

static void FreeWetLocalSourceBitmap( UWetTexture* Owner )
{
	FWaterHostState* State = FindWaterHostState(Owner);
	if( !State )
		return;
	delete[] State->LocalSourceBitmap;
	State->LocalSourceBitmap = NULL;
	RemoveWaterHostStateIfEmpty(Owner, State);
}

/*-----------------------------------------------------------------------------
	Package implementation.
-----------------------------------------------------------------------------*/


IMPLEMENT_PACKAGE(Fire);


/*----------------------------------------------------------------------------
	Random number and math functions.
----------------------------------------------------------------------------*/

// Shift-register random generator.
// based on trinomial x**63 + x**31 + 1.
// Period of about 2^63  - tested up to about 2^37.

inline BYTE SpeedRand()
{
	return hp2_rs_fire_speed_rand( FireTraceHooksForRust() );
}



/*----------------------------------------------------------------------------
	Initialization routines.
----------------------------------------------------------------------------*/

// Initialize all _global_ tables.

void InitTables()
{
		const unsigned long long TraceRngBefore = appGetRandTraceOrdinal();

	static INT  Initialized=0;
	if( !Initialized )
	{
		FAppRandTracePhaseScope StartupPhaseTrace( "fire_init_tables" );
		// Engine calls in C order: appSin per entry, then the 512 appRand draws.
		// Rust fills the phase tables, SpeedRand register and the fingerprint hook; the phase tables are copied back below for C++ readers.
		DOUBLE SineTable[256];
		for( INT t=0; t<256; t++ )
			SineTable[t] = appSin( ((FLOAT)t/256.f) * 6.2831853f );
		BYTE RngSeed[512];
		for( INT t=0; t<512; t++ )
			RngSeed[t] = (BYTE)(appRand() & 255);
		hp2_rs_fire_init_tables( FireTraceHooksForRust(), SineTable, 256, RngSeed, 512, TraceRngBefore, appGetRandTraceOrdinal() );
		hp2_rs_fire_read_phase_tables( PhaseTable, SignedPhaseTable, LightPhaseTable );
		// Now initialized;
		Initialized=1;
	}
}



/*----------------------------------------------------------------------------
	Fire calculation.
----------------------------------------------------------------------------*/

//
// Update fire.
//

void CalculateFire( BYTE* BitmapAddr, BYTE* RenderTable, DWORD Xdimension, DWORD Ydimension )
{
	guard(CalculateFire);
	hp2_rs_fire_calculate_fire( BitmapAddr, (size_t)Xdimension*Ydimension, RenderTable, 1024, Xdimension, Ydimension );
	unguard;
}

void CalcWrapFire( BYTE* BitmapAddr, BYTE* RenderTable, DWORD Xdimension, DWORD Ydimension )
{
	hp2_rs_fire_calc_wrap_fire( BitmapAddr, (size_t)Xdimension*Ydimension, RenderTable, 1024, Xdimension, Ydimension );
}

// Update special fire.

void CalcSlowFire( BYTE* BitmapAddr, BYTE* RenderTable, DWORD Xdimension, DWORD Ydimension )
{
	hp2_rs_fire_calc_slow_fire( BitmapAddr, (size_t)Xdimension*Ydimension, RenderTable, 1024, Xdimension, Ydimension );
}


/*----------------------------------------------------------------------------
	Add a spark to the array.
----------------------------------------------------------------------------*/

enum {  // Internal spark types.
	// special-case ones 
	SPARK_LissajX = SPARK_LASTTYPE , 
 	SPARK_LissajY        ,
	// Spawned ones: all above SPARK_TRANSIENTS are transient ones.
	ISPARK_TRANSIENTS    , 
	ISPARK_Drifter       ,
	ISPARK_DriftSlow     ,
	ISPARK_Faller        , 
	ISPARK_VShooter      ,     
	ISPARK_Drop          ,    
    ISPARK_Move          ,    
    ISPARK_SpawnedEel	 ,
	ISPARK_SpawnedTwirl	 ,
	ISPARK_SprinklerTwirl,
	ISPARK_Custom	     ,
	ISPARK_Graviton      ,
	ISPARK_SpawnedSperm  ,
};


static_assert( sizeof(FSpark) == sizeof(FireSpark), "FSpark and FireSpark must share layout for the (FireSpark*)Sparks casts" );

// Copies the sparks-slice scalar state into the Rust mirror. The spark array is passed separately.
static void FillSparkState( FireSparkState& State, const UFireTexture& Texture )
{
	State.texture          = (size_t)&Texture;
	State.u_size           = Texture.USize;
	State.v_size           = Texture.VSize;
	State.u_bits           = (BYTE)Texture.UBits;
	State.u_mask           = Texture.UMask;
	State.v_mask           = Texture.VMask;
	State.global_phase     = Texture.GlobalPhase;
	State.draw_phase       = Texture.DrawPhase;
	State.aux_phase        = Texture.AuxPhase;
	State.spark_type       = Texture.SparkType;
	State.draw_mode        = Texture.DrawMode;
	State.sparks_limit     = Texture.SparksLimit;
	State.active_spark_num = Texture.ActiveSparkNum;
	State.star_status      = Texture.StarStatus;
	State.pen_down_x       = Texture.PenDownX;
	State.pen_down_y       = Texture.PenDownY;
	State.fx_heat          = Texture.FX_Heat;
	State.fx_size          = Texture.FX_Size;
	State.fx_aux_size      = Texture.FX_AuxSize;
	State.fx_area          = Texture.FX_Area;
	State.fx_frequency     = Texture.FX_Frequency;
	State.fx_phase         = Texture.FX_Phase;
	State.fx_horiz_speed   = Texture.FX_HorizSpeed;
	State.fx_vert_speed    = Texture.FX_VertSpeed;
}

// Writes back the scalar state that the Rust sparks routines may change.
static void StoreSparkState( UFireTexture& Texture, const FireSparkState& State )
{
	Texture.ActiveSparkNum = State.active_spark_num;
	Texture.GlobalPhase    = State.global_phase;
	Texture.DrawPhase      = State.draw_phase;
	Texture.AuxPhase       = State.aux_phase;
	Texture.StarStatus     = State.star_status;
	Texture.PenDownX       = State.pen_down_x;
	Texture.PenDownY       = State.pen_down_y;
}

void UFireTexture::AddSpark( INT  MouseX, INT  MouseY )
{
	guard(UFireTexture::AddSpark);

	FireSparkState State;
	FillSparkState( State, *this );
	hp2_rs_fire_add_spark( FireTraceHooksForRust(), &State, (FireSpark*)Sparks.GetData(), (size_t)Sparks.Num(), MouseX, MouseY );
	StoreSparkState( *this, State );

	unguard;
}


void UFireTexture::CloseSpark( INT  MouseX, INT  MouseY)
{
	guard(UFireTexture::CloseSpark);

	// The close path ignores the mouse position.
	FireSparkState State;
	FillSparkState( State, *this );
	hp2_rs_fire_close_spark( FireTraceHooksForRust(), &State, (FireSpark*)Sparks.GetData(), (size_t)Sparks.Num() );
	StoreSparkState( *this, State );

	unguard;
}



/*----------------------------------------------------------------------------
	Spark deletion.
----------------------------------------------------------------------------*/

void UFireTexture::DeleteSparks( INT  SparkX, INT  SparkY, INT  AreaWidth)
{
	guard(UFireTexture::DeleteSparks);

	FireSparkState State;
	FillSparkState( State, *this );
	hp2_rs_fire_delete_sparks( FireTraceHooksForRust(), &State, (FireSpark*)Sparks.GetData(), (size_t)Sparks.Num(), SparkX, SparkY, AreaWidth );
	StoreSparkState( *this, State );

	unguard;
}


/*----------------------------------------------------------------------------
	Spark line drawing.
----------------------------------------------------------------------------*/

void UFireTexture::DrawSparkLine( INT  StartX, INT  StartY, INT  DestX, INT  DestY, INT  Density )
{
	guard(UFireTexture::DrawSparkLine);

	// Density is not read by the spark line path.
	FireSparkState State;
	FillSparkState( State, *this );
	hp2_rs_fire_draw_spark_line( FireTraceHooksForRust(), &State, (FireSpark*)Sparks.GetData(), (size_t)Sparks.Num(), StartX, StartY, DestX, DestY );
	StoreSparkState( *this, State );

	unguard;
}


/*----------------------------------------------------------------------------
	Fire painting.
----------------------------------------------------------------------------*/

// Spark-paint routine - the fire-specific part of the editor.

void UFireTexture::FirePaint( INT MouseX, INT MouseY, DWORD Buttons )
{
	guard(UFireTexture::FirePaint);

	FireSparkState State;
	FillSparkState( State, *this );
	hp2_rs_fire_fire_paint( FireTraceHooksForRust(), &State, (FireSpark*)Sparks.GetData(), (size_t)Sparks.Num(), MouseX, MouseY, Buttons );
	StoreSparkState( *this, State );

	unguard;
}




/*----------------------------------------------------------------------------
	Spark redrawing.
----------------------------------------------------------------------------*/



void UFireTexture::RedrawSparks()
{
	guard(UFireTexture::RedrawSparks);

	FireSparkState State;
	FillSparkState( State, *this );
	hp2_rs_fire_redraw_sparks( FireTraceHooksForRust(), &State, (FireSpark*)Sparks.GetData(), (size_t)Sparks.Num(), Mips(0).DataArray.GetData(), (size_t)Mips(0).DataArray.Num() );
	StoreSparkState( *this, State );

	unguard;
}



void UFireTexture::PostDrawSparks()
{
	guard(UFireTexture::PostDrawSparks);

	FireSparkState State;
	FillSparkState( State, *this );
	hp2_rs_fire_post_draw_sparks( FireTraceHooksForRust(), &State, (FireSpark*)Sparks.GetData(), (size_t)Sparks.Num(), Mips(0).DataArray.GetData(), (size_t)Mips(0).DataArray.Num() );
	StoreSparkState( *this, State );

	unguard;
}



/*---------------------------------------------------------------------------
	All water code
---------------------------------------------------------------------------*/

// WATER interpolated calculation. The update runs in hp2-fire (water.rs).

static_assert( sizeof(FDrop) == sizeof(FireDrop), "FDrop and FireDrop must share layout for the (FireDrop*)Drops casts" );

// Copies the scalar water state into the Rust mirror. Drops and tables are passed by pointer.
static void FillWaterState( FireWaterState& State, const UWaterTexture& Texture )
{
	State.u_size       = Texture.USize;
	State.v_size       = Texture.VSize;
	State.u_bits       = Texture.UBits;
	State.u_mask       = Texture.UMask;
	State.v_mask       = Texture.VMask;
	State.global_phase = Texture.GlobalPhase;
	State.draw_phase   = Texture.DrawPhase;
	State.num_drops    = Texture.NumDrops;
	State.drop_type    = Texture.DropType;
	State.wave_parity  = Texture.WaveParity;
	State.fx_frequency = Texture.FX_Frequency;
	State.fx_phase     = Texture.FX_Phase;
	State.fx_size      = Texture.FX_Size;
	State.fx_depth     = Texture.FX_Depth;
}

// Writes back the scalar state that the Rust water routines may change.
static void StoreWaterState( UWaterTexture& Texture, const FireWaterState& State )
{
	Texture.GlobalPhase = State.global_phase;
	Texture.DrawPhase   = State.draw_phase;
	Texture.NumDrops    = State.num_drops;
	Texture.WaveParity  = State.wave_parity;
}

void UWaterTexture::CalculateWater()
{
	guard(UWaterTexture::CalculateWater);

	FireWaterState State;
	FillWaterState( State, *this );
	hp2_rs_fire_calculate_water
	(
		FireTraceHooksForRust(),
		&State,
		&Mips(0).DataArray(0),
		(size_t)Mips(0).DataArray.Num(),
		GetSourceFields(),
		(size_t)(USize * VSize / 2),
		RenderTable,
		sizeof(RenderTable),
		WaveTable,
		sizeof(WaveTable)
	);
	StoreWaterState( *this, State );

	unguard;
}


//
//   Apply texture: simple use of the texels itself as displacement-lookup into another texture.
//

void UWetTexture::ApplyWetTexture()
{
	guard(UWetTexture::ApplyWetTexture);

    // Apply texture: SourceTexture to
    // the current (output) texture,
    // using displacement effects.

	// Any source texture selected yet ?
    if( SourceTexture == NULL ) return;

    BYTE* BitMapAddr     = &Mips(0).DataArray(0);     // pointer

	BYTE* SourceMapAddr;
	BYTE* LocalBitmap = FindWetLocalSourceBitmap(this);
	if( LocalBitmap )
		SourceMapAddr = LocalBitmap;
	else
		SourceMapAddr = &SourceTexture->Mips(0).DataArray(0);

	INT SourceLength = LocalBitmap ? USize * VSize : SourceTexture->Mips(0).DataArray.Num();

	FireWetLayout Layout;
	Layout.u_size = USize;
	Layout.v_size = VSize;
	Layout.u_bits = UBits;

	// SourceMapAddr == BitMapAddr selects the in-place (self-source) path in Rust.
	hp2_rs_fire_apply_wet_texture( FireTraceHooksForRust(), BitMapAddr, Mips(0).DataArray.Num(), SourceMapAddr, SourceLength, SourceMapAddr == BitMapAddr, Layout );

	unguard;
}



void UWaterTexture::WaterRedrawDrops()
{
	guard(UWaterTexture::WaterRedrawDrops);

	FireWaterState State;
	FillWaterState( State, *this );
	hp2_rs_fire_water_redraw_drops( FireTraceHooksForRust(), &State, (FireDrop*)Drops, MaxDrops, GetSourceFields(), (size_t)(USize * VSize / 2) );
	StoreWaterState( *this, State );

	unguard;
}



//
// Adding a drop to the Pool structure (edit-time).
//

void UWaterTexture::AddDrop( INT  DropX, INT  DropY )
{
	guard(UWaterTexture::AddDrop);

	FireWaterState State;
	FillWaterState( State, *this );
	hp2_rs_fire_add_drop( FireTraceHooksForRust(), &State, (FireDrop*)Drops, MaxDrops, DropX, DropY );
	StoreWaterState( *this, State );

	unguard;
}


// Delete a drop within a certain area in the image.
void UWaterTexture::DeleteDrops( INT  DropX, INT  DropY, INT  AreaWidth )
{
	guard(UWaterTexture::DeleteDrops);

	FireWaterState State;
	FillWaterState( State, *this );
	hp2_rs_fire_delete_drops( FireTraceHooksForRust(), &State, (FireDrop*)Drops, MaxDrops, DropX, DropY, AreaWidth );
	StoreWaterState( *this, State );

	unguard;
}


/*----------------------------------------------------------------------------
	Temp spark drawing (for editor)
----------------------------------------------------------------------------*/

void UFireTexture::TempDrawSpark (INT PosX, INT  PosY, INT Intensity )
{
	hp2_rs_fire_temp_draw_spark( FireTraceHooksForRust(), &Mips(0).DataArray(0), (size_t)Mips(0).DataArray.Num(), UMask, VMask, (BYTE)UBits, PosX, PosY, Intensity );
}


/*----------------------------------------------------------------------------
	Very non-sucking default palette.
----------------------------------------------------------------------------*/

void BlueLagunaPalette(UPalette* TargetPal)
{
	FireRgb Colors[256];
	hp2_rs_fire_blue_laguna_palette( Colors, 256 );
	for( INT i=0; i<256; i++ )
	{
		TargetPal->Colors(i).R = Colors[i].r;
		TargetPal->Colors(i).G = Colors[i].g;
		TargetPal->Colors(i).B = Colors[i].b;
	}
};


/*----------------------------------------------------------------------------
	UFractalTexture object implementation.
----------------------------------------------------------------------------*/

UFractalTexture::UFractalTexture()
{
	guard(UFractalTexture::UFractalTexture);

	// Mark the texture as parametric (so bitmap is not saved).	
	bParametric = 1;
	// Mark the texture as realtime   (so it is regenerated each tick).
	bRealtime   = 1; 
	// realtime uploads ONLY if Fractal texture animation is enabled.

	InitTables(); // Global LUTS, checks itself to see if already initzed.

	DrawPhase  =0;
	AuxPhase   =0;
    GlobalPhase=0;

	unguard;
}


void UFractalTexture::Init( INT  InUSize, INT  InVSize )
{
	guard(UFractalTexture::Init);

	VERIFY_CLASS_OFFSET(U,FractalTexture,UMask); // verifies correct mirroring..

	// Proper-powers-of-two verification.
	check((InUSize&(InUSize-1))==0); 
	check((InVSize&(InVSize-1))==0);

	// Init base class.
	UTexture::Init( InUSize, InVSize );

    unguard;
}


void UFractalTexture::PostLoad()
{
	Super::PostLoad();

	UMask = USize - 1;
	VMask = VSize - 1;
}

void UFractalTexture::Prime()
{
	guard(UFractalTexture::Prime);

	if (PrimeCurrent != 0) return;

	// See if animation disabled, if so warm up the animation to obtain reasonably looking static texture.
	if ( __Client && __Client->NoFractalAnim )
	{		
		// Suspend NoFractalAnim freezing temporarily.
		__Client->NoFractalAnim = 0;
		// Force a priming update of at least 48 frames.
		PrimeCount = Max( PrimeCount, (BYTE)48 );
		while( PrimeCurrent < PrimeCount )
		{
			PrimeCurrent++;
			ConstantTimeTick();
		}	
		__Client->NoFractalAnim = 1;
		// No more special uploading required.
		bRealtime = 0;
	}
	else
	{
		// Regular priming.
		Super::Prime();
	}
	unguard;
}


IMPLEMENT_CLASS(UFractalTexture);




/*----------------------------------------------------------------------------
	UFireTexture.
----------------------------------------------------------------------------*/

UFireTexture::UFireTexture() 
{
    // Init all non-serialized variables. 

	OldRenderHeat  =  -1; 
	AuxPhase       =   0;
	StarStatus	   =   1; // Assume some stars until, we find otherwise.

	// initialize the routines we should call for rising/nonrising fire;
	// one for Pentium/MMX es and one for Ppro/PII's

	//void (*MergePass)( INT Y, INT X, INT InnerX );

}

//
// ::Init  called only _once_ ,on creation in the editor)
//

void UFireTexture::Init( INT  InUSize, INT  InVSize )
{
	guard(UFireTexture::Init);

	VERIFY_CLASS_OFFSET(U,Bitmap,MaxColor);
	VERIFY_CLASS_OFFSET(U,Texture,BumpMap);
	VERIFY_CLASS_OFFSET(U,FireTexture,SparkType);

	// Init base class.
	UFractalTexture::Init( InUSize, InVSize );

	// Create a custom palette.
	Palette = new( GetOuter() )UPalette;
	for( INT i=0; i<256; i++ )
		new(Palette->Colors)FColor(i,i,0);
	BlueLagunaPalette(Palette);
	MipZero    = Palette->Colors(128);

	// initialize all to-be-serialized variables
    RenderHeat  = 220;
	ActiveSparkNum   =   0;
    SparkType   = SPARK_Blaze;
	SparksLimit = MAXSPARKSINIT; // inital maximum

	//Allocate SparksLimit sparks:
	Sparks.Add(SparksLimit);

	FX_Frequency  =  16;
	FX_Phase	  =  16;
	FX_Heat		  = 255;
	FX_Size		  =  96;
	FX_Area		  =  24;
	FX_HorizSpeed = 130;
	FX_VertSpeed  = 142;

	unguard;
}

void UFireTexture::Clear( DWORD ClearFlags )
{
	guard(UFireTexture::Clear);

	// Init to zero.
	UFractalTexture::Clear( ClearFlags );

	// Clear sparks.
	if( ClearFlags & TCLEAR_Temporal )
	{
		ActiveSparkNum=0;
		StarStatus=0;
	}

	unguard;
}

void UFireTexture::PostLoad()
{
	guard(UFireTexture::PostLoad);
	VERIFY_CLASS_SIZE(UBitmap);
	VERIFY_CLASS_SIZE(UTexture);
	VERIFY_CLASS_SIZE(UFireTexture);
	VERIFY_CLASS_SIZE(UWetTexture);
	VERIFY_CLASS_SIZE(UWaveTexture);
	VERIFY_CLASS_SIZE(UFractalTexture);

	// Call base class.
	Super::PostLoad();
	PolyFlags &= ~PF_Masked;

	// Make sure the texture has its _own_ copy of the palette.
#if COPYPALETTE
	if( ( Palette!=NULL ) && ( GetOuter()!=Palette->GetOuter() ) )
	{
		UPalette* NewPalette = new( GetOuter(), GetName() )UPalette;
		//NewPalette->Colors = Palette->Colors;
		for( INT i=0; i<256; i++ )
			new(NewPalette->Colors)FColor( Palette->Colors(i) );
		Palette = NewPalette->ReplaceWithExisting();
		MipZero = Palette->Colors(128);
		GCache.Flush();
	}
#endif

	// Fill the fire table.
	if( OldRenderHeat != RenderHeat )
	{
		hp2_rs_fire_fill_render_table( RenderHeat, RenderTable, 1024 );
		OldRenderHeat = RenderHeat;
	}

	// Expand/shrink the dynamic array to become
	// exactly SparksLimit elements.
	// Acts both at load time and edit time.
	
	if( Sparks.Num() != SparksLimit )
	{
		// Make sure not TOO big...
		if( SparksLimit > MAXSPARKSLIMIT )
			SparksLimit = MAXSPARKSLIMIT;
		else
		if( SparksLimit < MINSPARKSLIMIT )
			SparksLimit = MINSPARKSLIMIT;

		// New SparksLimit; current size is too big. 
		if( Sparks.Num() > SparksLimit )
		{
			// Try culling some transients if needed.
			if  (ActiveSparkNum > SparksLimit)
				for( INT t = (ActiveSparkNum-1); t>=0; t-- )
				{		
					if( Sparks(t).Type >= ISPARK_TRANSIENTS ) 
					{
						Sparks(t) = Sparks(--ActiveSparkNum);	
						// culled enough ?
						if( ActiveSparkNum<= SparksLimit ) break;
					}			 
				}

			// Adjust active sparks the crude way:
			ActiveSparkNum = Min( ActiveSparkNum, SparksLimit );
			// Now delete the empty tail. 
			Sparks.Remove( SparksLimit, Sparks.Num()-SparksLimit); 

		}
		else 
		// Too small: just expand the array.
		{
			Sparks.Add( SparksLimit - Sparks.Num() );
		}
		//Sparks.Debug(); 
	}

	unguard;
}



void UFireTexture::TouchTexture(INT UPos, INT VPos, FLOAT Magnitude)
{
	guard(UFireTexture::TouchTexture);

    DWORD SparkDest = (DWORD)(UPos + (VPos << UBits) );
    Mips(0).DataArray(SparkDest) = (BYTE) Magnitude;

	unguard;
}


void UFireTexture::ConstantTimeTick()
{
	guard(UFireTexture::ConstantTimeTick);
	if( __Client && (! __Client->NoFractalAnim) )
	{
		if( (USize>=8) && (VSize>=8) ) // safe sizes ?
		{
			// Call FireEngine update.

			RedrawSparks();

			if( bRising ) CalcWrapFire( &Mips(0).DataArray(0), RenderTable, USize, VSize );
			else      CalcSlowFire( &Mips(0).DataArray(0), RenderTable, USize, VSize );

			PostDrawSparks();
		}
	}

	unguard;
}



void UFireTexture::MousePosition( DWORD Buttons, FLOAT X, FLOAT Y )
{
	guard(UFireTexture::MousePosition);

	// Call FireEngine update. (adds or removes sparks in spark array)
	FirePaint( appRound(X), appRound(Y), Buttons );

	// Mouse 'torch' animation. Won't work on still cursor.
	if( 1 )
	{
		TempDrawSpark(appRound(X)  ,appRound(Y),255);
		TempDrawSpark(appRound(X)+1,appRound(Y),255);
		TempDrawSpark(appRound(X)-1,appRound(Y),255);
	}
    else
    {
    }

	unguard;
}



void UFireTexture::Click( DWORD Buttons, FLOAT X, FLOAT Y )
{
	guard(UFireTexture::Click);

	// Call FireEngine update.
	FirePaint( appRound(X), appRound(Y), Buttons );

	// Mouse 'torch' animation.
	if( 1 )
	{
		Mips(0).DataArray((INT)X + (INT)Y * USize) = 255;
	}
    else
    {
        //SparkType = TouchType;
        //AddSpark( X, Y);
    }
	unguard;
}

void UFireTexture::Serialize( FArchive& Ar )
{
	guard(UFireTexture::Serialize);
	Super::Serialize( Ar );

    // Clean up & minimize our spark array for saving to disk:
	if( Ar.IsSaving() ) 
	{
		// Delete the transients, compact the spark array.
		for( INT t=ActiveSparkNum-1; t>=0; t-- )
			if( Sparks(t).Type >= ISPARK_TRANSIENTS ) 
				Sparks(t) = Sparks(--ActiveSparkNum);	

		// Deflate the empty tail.
		if( ActiveSparkNum < Sparks.Num() )
			Sparks.Remove( ActiveSparkNum, Sparks.Num()-ActiveSparkNum ); 
	}
	Ar << Sparks;

	// Expand the compacted array again. Sparks.Num() should always be equal to SparksLimit !
	if( Sparks.Num() < SparksLimit )
		Sparks.Add( SparksLimit - Sparks.Num() );

	unguard;
}


IMPLEMENT_CLASS(UFireTexture);


/*----------------------------------------------------------------------------
	UWaterTexture object implementation.
----------------------------------------------------------------------------*/

BYTE* UWaterTexture::GetSourceFields() const
{
	return FindWaterSourceFields(this);
}


UWaterTexture::UWaterTexture()
{
	guard(UWaterTexture::UWaterTexture);

	// Fill the algorithm lookup table.
    for( INT t=0; t<(1024+512); t++ )
		WaveTable[t] = Clamp( (t/2 - 256) + ( (t-512)< 256), 0, 255 );

	// Init most non-serialized vars.
	OldWaveAmp     = -1; 
	WaveParity     =  0;
	SourceFields   =  0;

	unguard;
}


// Init. Called only _once_, on creation of texture in editor.

void UWaterTexture::Init( INT  InUSize, INT  InVSize )
{
	guard(UWaterTexture::Init);

	VERIFY_CLASS_OFFSET(U,WaterTexture,DropType);

	// Init base class.
	UFractalTexture::Init( InUSize, InVSize );

	// One-time initialization of *serialized* parameters.
    NumDrops =   0;
    DropType =   1;        
	WaveAmp  = 128;

	// FX specifics
	FX_Frequency	= 010;
	FX_Phase		= 010;
	FX_Amplitude	= 255;
	FX_Speed		= 255;
	FX_Radius		= 128;
	FX_Size			= 30;
	FX_Depth		= 255;
	FX_Time         = 30;

	unguard;
}


void UWaterTexture::PostLoad()
{
	guard(UWaterTexture::PostLoad);
	Super::PostLoad();

	BYTE* Fields = GetSourceFields();
	if( Fields == NULL )
	{
		const INT FieldSize = USize * VSize / 2;
		Fields = AllocateWaterSourceFields(this, FieldSize);
		// initialize water to average height.
		for( INT i=0; i<FieldSize; i++ )
			Fields[i] = 128;
	}

	unguard;
}



void UWaterTexture::TouchTexture(INT UPos, INT VPos, FLOAT Magnitude)
{
	guard(UWaterTexture::TouchTexture);

	FireWaterState State;
	FillWaterState( State, *this );
	hp2_rs_fire_touch_texture( FireTraceHooksForRust(), &State, GetSourceFields(), (size_t)(USize * VSize / 2), UPos, VPos, Magnitude );

	unguard;
}




void UWaterTexture::Destroy()
{
	guard(UWaterTexture::Destroy);

	FreeWaterSourceFields(this);
	SourceFields = 0;

	Super::Destroy(); // must call base class' destroy.
	unguard;
}


void UWaterTexture::Clear( DWORD ClearFlags )
{
	guard(UWaterTexture::Clear);

	// Init to zero.
	UFractalTexture::Clear( ClearFlags );

	// Clear fields.
	if( ClearFlags & TCLEAR_Bitmap )
	{
		BYTE* Fields = GetSourceFields();
		for( INT i=0; i< USize * VSize / 2; i++ )
			Fields[i] = 128;
	}

	// Clear drops.
	if( ClearFlags & TCLEAR_Temporal )
		NumDrops=0;

	unguard;
}


void UWaterTexture::WaterPaint( INT X, INT Y, DWORD Buttons)
{
	guard(UWaterTexture::WaterPaint);

	FireWaterState State;
	FillWaterState( State, *this );
	hp2_rs_fire_water_paint( FireTraceHooksForRust(), &State, (FireDrop*)Drops, MaxDrops, X, Y, Buttons );
	StoreWaterState( *this, State );

	unguard;
}


void UWaterTexture::MousePosition( DWORD Buttons, FLOAT X, FLOAT Y )
{
	guard(UWaterTexture::MousePosition);

	WaterPaint(appRound(X),appRound(Y),Buttons);
	
	unguard;
}



void UWaterTexture::Click( DWORD Buttons, FLOAT X, FLOAT Y )
{
	guard(UWaterTexture::Click);

	INT MouseX=(INT) X, MouseY=(INT) Y;

	// Paint on the water.
    if( Buttons & MOUSE_Left )
	{
        AddDrop( MouseX, MouseY);
	}

    if( Buttons & MOUSE_Right )
        DeleteDrops( MouseX, MouseY, DEL_RANGE);

	unguard;
}



IMPLEMENT_CLASS(UWaterTexture);


/*----------------------------------------------------------------------------
	UWaveTexture object implementation.
----------------------------------------------------------------------------*/

UWaveTexture::UWaveTexture()
{
	guard(UWaveTexture::UWaveTexture);


	unguard;
}

void UWaveTexture::Init( INT  InUSize, INT  InVSize )
{
	guard(UWaveTexture::Init);

	VERIFY_CLASS_OFFSET(U,WaveTexture,BumpMapAngle);

	// Init base class.
	UWaterTexture::Init( InUSize, InVSize );

	BumpMapAngle = 170;
	BumpMapLight = 50;

	PhongRange = 180;
	PhongSize  = 32;

	// Create a custom palette.	
	Palette = new( GetOuter() )UPalette;
	for( INT i=0; i<256; i++ )
		new(Palette->Colors)FColor(0,0,0);
	BlueLagunaPalette(Palette);
	MipZero = Palette->Colors(128);

	unguard;
}

void UWaveTexture::PostLoad()
{
	guard(UWaveTexture::PostLoad);
	Super::PostLoad();

	// Make sure the texture has its _own_ copy of the palette.
#if COPYPALETTE	
	if( ( Palette!=NULL ) && ( GetOuter()!=Palette->GetOuter() ) )
	{
		UPalette* NewPalette = new( GetOuter(), GetName() )UPalette;
		//NewPalette->Colors = Palette->Colors;
		for( INT i=0; i<256; i++ )
			new(NewPalette->Colors)FColor( Palette->Colors(i) );
		Palette = NewPalette->ReplaceWithExisting();
		MipZero = Palette->Colors(128);
		GCache.Flush();
	}
#endif
	
	// Recalculate the rendering LUT
	// Depends on: WaveAmp, PhongRange, and PhongSize.
	SetWaveLight(); //TODO: Check params before superfluous updates ??

	unguard;
}

void UWaveTexture::Clear( DWORD ClearFlags )
{
	guard(UWaveTexture::Clear);

	// Init to zero.
	UWaterTexture::Clear( ClearFlags );

	// Clear fields.
	if( ClearFlags & TCLEAR_Bitmap )
	{
		BYTE* Fields = GetSourceFields();
		for( INT i=0; i< USize * VSize / 2; i++ )
			Fields[i] = 128;
	}

	// Clear drops.
	if( ClearFlags & TCLEAR_Temporal )
		NumDrops=0;

	unguard;
}

void UWaveTexture::ConstantTimeTick()
{
	guard(UWaveTexture::ConstantTimeTick);
	if( __Client && (! __Client->NoFractalAnim) )
	{
		if( (USize>=8) && (VSize>=8) ) // safe sizes ?
		{
			WaterRedrawDrops();
			CalculateWater(); 
		}
	}
	unguard;
}


void UWaveTexture::SetWaveLight() 
{
	guard(UWaveTexture::SetWaveLight);

	FireWaveLight Params;
	Params.wave_amp = WaveAmp;
	Params.bump_map_light = BumpMapLight;
	Params.bump_map_angle = BumpMapAngle;
	Params.phong_range = PhongRange;
	Params.phong_size = PhongSize;

	hp2_rs_fire_wave_set_wave_light( FireTraceHooksForRust(), Params, RenderTable, sizeof(RenderTable) );

	unguard;
}


IMPLEMENT_CLASS(UWaveTexture);



/*----------------------------------------------------------------------------
	UWetTexture implementation.
----------------------------------------------------------------------------*/

UWetTexture::UWetTexture()
{
	guard(UWetTexture::UWetTexture);
	OldSourceTex = NULL;
	LocalSourceBitmap = 0;
	unguard;
}

void UWetTexture::Init( INT  InUSize, INT  InVSize )
{
	guard(UWetTexture::Init);

	// Init base class.
	UWaterTexture::Init( InUSize, InVSize );

	// Create a custom palette.
	Palette = new( GetOuter() )UPalette;
	for( INT i=0; i<256; i++ )
		new(Palette->Colors)FColor(i,i,i);
	BlueLagunaPalette(Palette);
	MipZero = Palette->Colors(128);

	unguard;
}

void UWetTexture::PostLoad()
{
	guard(UWetTexture::PostLoad);
	Super::PostLoad();
	LocalSourceBitmap = 0;

	if( !SourceTexture ) return;

	// Make sure the source texture data is in memory !
	FTextureInfo Info; 
	if( SourceTexture != this ) 
	{
		 SourceTexture->Lock( Info, 0.0, 0, NULL );
		 SourceTexture->Unlock( Info );
	}

	// Make sure this is the same size as our displacement-waves texture.
	if( ( SourceTexture->UBits  != UBits) || (SourceTexture->VBits  != VBits)  )
	{

		// Size discrepancy; try to recover a source texture by upsampling..
		INT UScaler = UBits - SourceTexture->UBits;
		INT VScaler = VBits - SourceTexture->VBits;

		if( (UScaler>=0 ) && (VScaler>=0) )
		{
			FreeWetLocalSourceBitmap(this);
			BYTE* LocalBitmap = AllocateWetLocalSourceBitmap(this, USize * VSize);
			BYTE* SourceMapAddr  = &SourceTexture->Mips(0).DataArray(0);

			FireWetResample Params;
			Params.u_size = USize;
			Params.v_size = VSize;
			Params.u_scaler = UScaler;
			Params.v_scaler = VScaler;
			Params.source_u_bits = SourceTexture->UBits;
			hp2_rs_fire_wet_resample_source( FireTraceHooksForRust(), LocalBitmap, USize * VSize, SourceMapAddr, SourceTexture->Mips(0).DataArray.Num(), Params );
		}
		else 
		{
			SourceTexture = NULL; // Give up trying to recover a valid sourcetexture.
		}
	}
	else
	{
		// Sizes match, may have changed to a fullsized sourcetexture.
		FreeWetLocalSourceBitmap(this);
	}

	if( (SourceTexture) && ( SourceTexture != OldSourceTex ) )
	{
		// Update palette if new source texture selected.
		Palette = SourceTexture->Palette; 
	}
	OldSourceTex = SourceTexture;

	// Recalculate the rendering LUT if needed.
	if( WaveAmp != OldWaveAmp )
	{
		SetRefractionTable();
	}
	OldWaveAmp = WaveAmp;

	unguard;
}


void UWetTexture::Clear( DWORD ClearFlags )
{
	guard(UWetTexture::Clear);

	// Init to zero.
	UWaterTexture::Clear( ClearFlags );

	// Clear fields.
	if( ClearFlags & TCLEAR_Bitmap )
	{
		BYTE* Fields = GetSourceFields();
		for( INT i=0; i< USize * VSize / 2; i++ )
			Fields[i] = 128;
	}

	// Clear drops.
	if( ClearFlags & TCLEAR_Temporal )
		NumDrops=0;

	unguard;
}

void UWetTexture::ConstantTimeTick()
{
	guard(UWetTexture::ConstantTimeTick);
	
	if( __Client && (! __Client->NoFractalAnim) )
	{
		if( SourceTexture && USize>=8 && VSize>=8 ) // safe sizes ?
		{
			// Make sure referred textures are updated also.
			FTextureInfo Info; 
			if( SourceTexture != this )  
			{
				SourceTexture->Lock( Info, GetLastUpdateTime(), 0, NULL );
				SourceTexture->Unlock( Info );
				//SourceTexture->Update(LastUpdateTime);
			}

			//Update the water.
			WaterRedrawDrops();
			CalculateWater();  
			ApplyWetTexture(); 
		}
	}
	unguard;
}

//
// Called from PostLoad  - for Wet texture it's simply linear.
//

void UWetTexture::SetRefractionTable()
{
	guard(UWetTexture::SetRefractionTable);
	hp2_rs_fire_wet_set_refraction_table( FireTraceHooksForRust(), WaveAmp, RenderTable, sizeof(RenderTable) );
	unguard;
}

void UWetTexture::Destroy()
{
	guard(UWetTexture::Destroy);

	FreeWetLocalSourceBitmap(this);
	LocalSourceBitmap = 0;

	// Must call direct parents' class destroy.	
	Super::Destroy();
	unguard;
}



IMPLEMENT_CLASS(UWetTexture);

/*----------------------------------------------------------------------------
	UIceTexture object implementation.
----------------------------------------------------------------------------*/

UIceTexture::UIceTexture()
{
	guard(UIceTexture::UIceTexture);
	
	MasterCount=0.0f;
	OldUDisp =   -1;
	OldVDisp =   -1;

	OldSourceTex = NULL;
	OldGlassTex = NULL;

	LocalSourceBitmap = NULL;

	ForceRefresh = 1;

	unguard;
}

void UIceTexture::PostLoad()
{
	guard(UIceTexture::PostLoad);
	Super::PostLoad();

	// Validate all sizes: minima & matching requirements.
	if( SourceTexture )  
	{
		// Make sure the source texture data is loaded.
		FTextureInfo Info; 
		if( SourceTexture != this ) 
		{
			SourceTexture->Lock( Info, 0.0, 0, NULL );
			SourceTexture->Unlock( Info );
		}

		if  (  ((USize<8) || (VSize<8))  )
		{
			SourceTexture = NULL;
			GlassTexture =  NULL;
		}
		else
		{
			UBOOL  WrongSizedSource = ((  SourceTexture->USize != USize ) || (  SourceTexture->VSize != VSize ) );
			UBOOL  WrongSizedGlass = true;
			if( GlassTexture ) 
			{
				WrongSizedGlass = ((  GlassTexture->USize  != USize ) || (  GlassTexture->VSize  != VSize ) );
				if( WrongSizedGlass ) GlassTexture = NULL;
			}

			if( ( (!WrongSizedSource) && (!WrongSizedGlass) ) )
			{
				LocalSourceBitmap = NULL;
			}
			else //	Static fake texture created if( WrongSizedSource || WrongSizedGlass ) 
			{
				// If either glass or source texture size is invalid, no longer animate 
				// but do copy whatever's in the source texture to be sure there's 
				// something valid on display..... and upsample if necessary.
				// Try to copy source texture to current texture.
				// Size changed? Try to recover a source texture by upsampling.
			
				INT UScaler = UBits - SourceTexture->UBits;
				INT VScaler = VBits - SourceTexture->VBits;

				if( (UScaler >=0) && (VScaler>=0) )
				{
					LocalSourceBitmap = &Mips(0).DataArray(0);  // Our static output image.
					BYTE* SourceMapAddr = &SourceTexture->Mips(0).DataArray(0);

					// Copy, with optional scaling, to our first mipmap (pixel loop in Rust).
					FireIceResample Resample;
					Resample.u_size     = USize;
					Resample.v_size     = VSize;
					Resample.u_scaler   = UScaler;
					Resample.v_scaler   = VScaler;
					Resample.src_u_bits = SourceTexture->UBits;
					hp2_rs_fire_ice_resample_source( FireTraceHooksForRust(), &Resample, LocalSourceBitmap, Mips(0).DataArray.Num(), SourceMapAddr, SourceTexture->Mips(0).DataArray.Num() );
				}
				else
				{
					SourceTexture = NULL; // Give up trying to rebuild a sourcetexture.
				}
			}
		}
	}

	if( (SourceTexture) && ( SourceTexture != OldSourceTex ) )
	{
		Palette = SourceTexture->Palette; // Make sure palette pointer gets updated.
		ForceRefresh = 1;
	}
	OldSourceTex = SourceTexture;

	if( GlassTexture != OldGlassTex ) 
		ForceRefresh = 1;
	OldGlassTex = GlassTexture;

	unguard;
}

void UIceTexture::Init( INT  InUSize, INT  InVSize )
{
	guard(UIceTexture::Init);

	// Init base class.
	UFractalTexture::Init( InUSize, InVSize );

	UDisplace=0;
	VDisplace=0;

	HorizPanSpeed = 128;
	VertPanSpeed  = 128;

	PanningStyle = SLIDE_Linear;
	Frequency    = 11;
	Amplitude    = 44;

	MoveIce=1;  // Move around the ICE if 1, TEXTURE if 0.

	// Create a dummy palette.
	Palette = new( GetOuter() )UPalette;
	for( INT i=0; i<256; i++ )
		new(Palette->Colors)FColor(i,i,i);
	MipZero = Palette->Colors(128);

	unguard;
}

void UIceTexture::Clear( DWORD ClearFlags )
{
	guard(UIceTexture::Clear);

	// Init to zero.
	UFractalTexture::Clear( ClearFlags );
	
	unguard;
}

void UIceTexture::ConstantTimeTick()
{
	guard(UIceTexture::ConstantTimeTick);
	if( __Client && (! __Client->NoFractalAnim) )
	{
		// Make sure referred textures are updated/loaded also.
		if( SourceTexture )
		{
			FTextureInfo Info; 
			if( SourceTexture != this ) 
			{
				SourceTexture->Lock( Info, GetLastUpdateTime(), 0, NULL );
				SourceTexture->Unlock( Info );
				//SourceTexture->Update(LastUpdateTime);
			}
		}
		if( GlassTexture )
		{
			FTextureInfo Info; 
			if( GlassTexture != this ) 
			{
				GlassTexture->Lock( Info, GetLastUpdateTime(), 0, NULL );
				GlassTexture->Unlock( Info );
				//GlassTexture->Update(LastUpdateTime);
			}
		}
		// Simulated 'deltaseconds' for 120 fps.
		RenderIce(1.0f/120.0f );
	}
	unguard;
}

// Full override of Tick().
// Ice is non-iterative so its panning can be synchronized to 
// any real-time speed.

void UIceTexture::Tick(FLOAT DeltaSeconds)  //"raw tick" override
{
	guard(UIceTexture::Tick);
	if( __Client && (! __Client->NoFractalAnim) )
	{
		// Revert to parent Tick() if panning stile is 'FrameSynced'. 
		if( TimeMethod == 0 ) 
		{	
			// Call parent, which might call UIceTexture::ConstantTimeTick 
			UFractalTexture::Tick(DeltaSeconds); 
			return;
		}

		// Make sure referred textures are updated/loaded also.
		if( SourceTexture )
		{
			FTextureInfo Info; 
			if( SourceTexture != this ) 
			{
				SourceTexture->Lock( Info, 0.0, 0, NULL );
				SourceTexture->Unlock( Info );
			}
		}
		if( GlassTexture )
		{
			FTextureInfo Info; 
			if( GlassTexture != this )
			{
				GlassTexture->Lock( Info, 0.0, 0, NULL );
				GlassTexture->Unlock( Info );
			}
		}
		RenderIce(DeltaSeconds);	
	}
	unguard;
}

void UIceTexture::MoveIcePosition(FLOAT DeltaTime)
{
	guard(UIceTexture::MoveIcePosition);

	FireIceState State;
	State.panning_style       = PanningStyle;
	State.horiz_pan_speed     = HorizPanSpeed;
	State.vert_pan_speed      = VertPanSpeed;
	State.frequency           = Frequency;
	State.amplitude           = Amplitude;
	State.move_ice            = MoveIce ? 1 : 0;
	State.local_source_bitmap = LocalSourceBitmap != NULL;
	State.u_bits              = UBits;
	State.force_refresh       = ForceRefresh;
	State.master_count        = MasterCount;
	State.u_displace          = UDisplace;
	State.v_displace          = VDisplace;
	State.u_position          = UPosition;
	State.v_position          = VPosition;
	State.old_u_disp          = OldUDisp;
	State.old_v_disp          = OldVDisp;
	State.u_size              = USize;
	State.v_size              = VSize;
	State.u_mask              = UMask;
	State.v_mask              = VMask;

	hp2_rs_fire_move_ice_position( FireTraceHooksForRust(), &State, DeltaTime );

	MasterCount = State.master_count;
	UDisplace   = State.u_displace;
	VDisplace   = State.v_displace;
	UPosition   = State.u_position;
	VPosition   = State.v_position;

	unguard;
}

void UIceTexture::BlitTexIce()
{
	guard(UIceTexture::BlitTexIce);

	FireIceState State;
	State.panning_style       = PanningStyle;
	State.horiz_pan_speed     = HorizPanSpeed;
	State.vert_pan_speed      = VertPanSpeed;
	State.frequency           = Frequency;
	State.amplitude           = Amplitude;
	State.move_ice            = MoveIce ? 1 : 0;
	State.local_source_bitmap = LocalSourceBitmap != NULL;
	State.u_bits              = UBits;
	State.force_refresh       = ForceRefresh;
	State.master_count        = MasterCount;
	State.u_displace          = UDisplace;
	State.v_displace          = VDisplace;
	State.u_position          = UPosition;
	State.v_position          = VPosition;
	State.old_u_disp          = OldUDisp;
	State.old_v_disp          = OldVDisp;
	State.u_size              = USize;
	State.v_size              = VSize;
	State.u_mask              = UMask;
	State.v_mask              = VMask;

	BYTE* BitmapData   = &Mips(0).DataArray(0);
	INT   BitmapLength = Mips(0).DataArray.Num();
	BYTE* SourceData   = SourceTexture ? &SourceTexture->Mips(0).DataArray(0) : NULL;
	INT   SourceLength = SourceTexture ? SourceTexture->Mips(0).DataArray.Num() : 0;
	BYTE* GlassData    = GlassTexture ? &GlassTexture->Mips(0).DataArray(0) : NULL;
	INT   GlassLength  = GlassTexture ? GlassTexture->Mips(0).DataArray.Num() : 0;

	hp2_rs_fire_blit_tex_ice( FireTraceHooksForRust(), &State, BitmapData, BitmapLength, SourceData, SourceLength, GlassData, GlassLength );

	unguard;
}

void UIceTexture::BlitIceTex()
{
	guard(UIceTexture::BlitIceTex);

	FireIceState State;
	State.panning_style       = PanningStyle;
	State.horiz_pan_speed     = HorizPanSpeed;
	State.vert_pan_speed      = VertPanSpeed;
	State.frequency           = Frequency;
	State.amplitude           = Amplitude;
	State.move_ice            = MoveIce ? 1 : 0;
	State.local_source_bitmap = LocalSourceBitmap != NULL;
	State.u_bits              = UBits;
	State.force_refresh       = ForceRefresh;
	State.master_count        = MasterCount;
	State.u_displace          = UDisplace;
	State.v_displace          = VDisplace;
	State.u_position          = UPosition;
	State.v_position          = VPosition;
	State.old_u_disp          = OldUDisp;
	State.old_v_disp          = OldVDisp;
	State.u_size              = USize;
	State.v_size              = VSize;
	State.u_mask              = UMask;
	State.v_mask              = VMask;

	BYTE* BitmapData   = &Mips(0).DataArray(0);
	INT   BitmapLength = Mips(0).DataArray.Num();
	BYTE* SourceData   = SourceTexture ? &SourceTexture->Mips(0).DataArray(0) : NULL;
	INT   SourceLength = SourceTexture ? SourceTexture->Mips(0).DataArray.Num() : 0;
	BYTE* GlassData    = GlassTexture ? &GlassTexture->Mips(0).DataArray(0) : NULL;
	INT   GlassLength  = GlassTexture ? GlassTexture->Mips(0).DataArray.Num() : 0;

	hp2_rs_fire_blit_ice_tex( FireTraceHooksForRust(), &State, BitmapData, BitmapLength, SourceData, SourceLength, GlassData, GlassLength );

	unguard;
}

void UIceTexture::RenderIce(FLOAT DeltaTime)
{
	guard(UIceTexture::RenderIce);

	// Safety & update checks.
	if( (GlassTexture == NULL) || (SourceTexture == NULL) ) return;

	MoveIcePosition(DeltaTime);

	// Skip update if last position == new position.
	if( (appRound(UPosition) == OldUDisp)
		&& (appRound(VPosition) == OldVDisp)
		&& (! ForceRefresh) )
		return;

	OldUDisp = appRound( UPosition );
	OldVDisp = appRound( VPosition );

	// Actual redrawing.
	if  (! MoveIce ) BlitIceTex();
		 else        BlitTexIce();

	ForceRefresh=0;

	unguard;
}

void UIceTexture::MousePosition( DWORD Buttons, FLOAT X, FLOAT Y )
{
	guard(UIceTexture::MousePosition);

	static FLOAT  MouseLastU=0.0f;
	static FLOAT  MouseLastV=0.0f;

	if( Buttons & MOUSE_Left )
	{ 
		UDisplace +=(MouseLastU - X);
		VDisplace +=(MouseLastV - Y);

		UPosition = UDisplace;
		VPosition = VDisplace;
	}

	MouseLastU = X;
	MouseLastV = Y;

	unguard;
}

void UIceTexture::Click( DWORD Buttons, FLOAT X, FLOAT Y )
{
	guard(UIceTexture::Click);
	unguard;
}

void UIceTexture::Destroy()
{
	guard(UWetTexture::Destroy);

	// Must call base class' destroy.
	Super::Destroy();
	unguard;
}



IMPLEMENT_CLASS(UIceTexture);

/*----------------------------------------------------------------------------
	The End.
----------------------------------------------------------------------------*/
