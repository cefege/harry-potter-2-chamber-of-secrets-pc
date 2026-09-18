/*=============================================================================
	UnScript.cpp: UnrealScript engine support code.
	Copyright 1997-1999 Epic Games, Inc. All Rights Reserved.

Description:
	UnrealScript execution and support code.

Revision history:
	* Created by Tim Sweeney
=============================================================================*/

#include "EnginePrivate.h"
#include "UnRender.h"
#include "UnNet.h"
#include "UnSkeletalMesh.h"
#include "UnLinker.h"
#include "HP2TraceHooks.h"


extern void HP2ActorTransitionPostInitExecution( AActor* Actor );


/*-----------------------------------------------------------------------------
	Tim's physics modes.
-----------------------------------------------------------------------------*/

FLOAT Splerp( FLOAT F )
{
	FLOAT S = Square(F);
	return (1.0f/16.0f)*S*S - (1.0f/2.0f)*S + 1;
}



// 
// Generalize animation retrieval to work for both skeletal meshes (animation sits in Actor->SkelAnim->AnimSeqs) and
// classic meshes (Mesh->AnimSeqs) For backwards compatibility....
//
FMeshAnimSeq* AActor::GetAnim( FName SequenceName)
{
	guard(AActor::GetAnim);
	if( !Mesh->IsA(USkeletalMesh::StaticClass()) ) // Classic mesh ?
	{
		return Mesh->GetAnimSeq( SequenceName );
	}
	else
	{
		if(!SkelAnim)
			return ((USkeletalMesh*)Mesh)->DefaultAnimation->GetAnimSeq( SequenceName );
		else
			return SkelAnim->GetAnimSeq( SequenceName );
	}
	unguard;
} 
//
// Interpolating along a path.
//
/*
void AActor::physPathing( FLOAT DeltaTime )
{
	guard(AActor::physPathing);

	// Linear interpolate from Target to Target.Next.
	while( PhysRate!=0.0f && bInterpolating && DeltaTime>0.0f )
	{
		// Find destination interpolation point, if any.
		AInterpolationPoint* Dest = Cast<AInterpolationPoint>( Target );

		// Compute rate modifier.
		FLOAT RateModifier = 1.0f;
		if( Dest && Dest->Next )
			RateModifier = Dest->RateModifier * (1.0f - PhysAlpha) + Dest->Next->RateModifier * PhysAlpha;

		// Update level slomo.
		Level->TimeDilation = Dest->GameSpeedModifier * (1.0f - PhysAlpha) + Dest->Next->GameSpeedModifier * PhysAlpha;

		// Update screenflash and FOV.
		if( IsA(APlayerPawn::StaticClass()) )
		{
			((APlayerPawn*)this)->FlashScale = FVector(1.f,1.f,1.f)*(((APlayerPawn*)this)->DesiredFlashScale = (Dest->ScreenFlashScale * (1.0f - PhysAlpha) + Dest->Next->ScreenFlashScale * PhysAlpha));
			((APlayerPawn*)this)->FlashFog   = ((APlayerPawn*)this)->DesiredFlashFog   = (Dest->ScreenFlashFog   * (1.0f - PhysAlpha) + Dest->Next->ScreenFlashFog   * PhysAlpha);
			((APlayerPawn*)this)->FovAngle                                             = (Dest->FovModifier      * (1.0f - PhysAlpha) + Dest->Next->FovModifier      * PhysAlpha) * ((APlayerPawn*)GetClass()->GetDefaultObject())->FovAngle;
		}

		// Update alpha.
		FLOAT OldAlpha  = PhysAlpha;
		FLOAT DestAlpha = PhysAlpha + PhysRate * RateModifier * DeltaTime;
		PhysAlpha       = Clamp( DestAlpha, 0.f, 1.f );

		// Move and rotate.
		if( Dest && Dest->Next )
		{
			FCheckResult Hit;
			FVector NewLocation;
			FRotator NewRotation;
			if( Dest->Prev && Dest->Next->Next )
			{
				// Cubic spline interpolation.
				FLOAT W0 = Splerp(PhysAlpha+1.0f);
				FLOAT W1 = Splerp(PhysAlpha+0.0f);
				FLOAT W2 = Splerp(PhysAlpha-1.0f);
				FLOAT W3 = Splerp(PhysAlpha-2.0f);
				FLOAT RW = 1.0f / (W0 + W1 + W2 + W3);
				NewLocation = (W0*Dest->Prev->Location + W1*Dest->Location + W2*Dest->Next->Location + W3*Dest->Next->Next->Location)*RW;
				NewRotation = (W0*Dest->Prev->Rotation + W1*Dest->Rotation + W2*Dest->Next->Rotation + W3*Dest->Next->Next->Rotation)*RW;
			}
			else
			{
				// Linear interpolation.
				FLOAT W0 = 1.0f - PhysAlpha;
				FLOAT W1 = PhysAlpha;
				NewLocation = W0*Dest->Location + W1*Dest->Next->Location;
				NewRotation = W0*Dest->Rotation + W1*Dest->Next->Rotation;
			}
			GetLevel()->MoveActor( this, NewLocation - Location, NewRotation, Hit );
			if( IsA(APawn::StaticClass()) )
				((APawn*)this)->ViewRotation = Rotation;
		}

		// If overflowing, notify and go to next place.
		if( PhysRate>0.0f && DestAlpha>1.0f )
		{
			PhysAlpha = 0.0f;
			DeltaTime *= (DestAlpha - 1.0f) / (DestAlpha - OldAlpha);
			if( Target )
			{
				Target->eventInterpolateEnd(this);
				eventInterpolateEnd(Target);
				if( Dest )
				{
					do
					{
						Target = Dest->Next;
						Dest = Cast<AInterpolationPoint>( Target );
					} while( Dest && Dest->bSkipNextPath );
				}
			}
		}
		else if( PhysRate<0.0f && DestAlpha<0.0f )
		{
			PhysAlpha = 1.0f;
			DeltaTime *= (0.0f - DestAlpha) / (OldAlpha - DestAlpha);
			if( Target )
			{
				Target->eventInterpolateEnd(this);
				eventInterpolateEnd(Target);
				if( Dest )
				{
					do
					{
						Target = Dest->Prev;
						Dest = Cast<AInterpolationPoint>( Target );
					} while( Dest && Dest->bSkipNextPath );
				}
			}
			eventInterpolateEnd(NULL);
		}
		else DeltaTime=0.0f;
	};
	unguard;
}
*/
//
// Moving brush.
//
// HP2 mover diagnostics: HP2_MOVER_DEBUG=1 enables mover freeze logging.
static INT GMoverDebugLog = -1;
static INT MoverDebugEnabled()
{
	if( GMoverDebugLog < 0 )
	{
		const char* Flag = getenv( "HP2_MOVER_DEBUG" );
		GMoverDebugLog = (Flag && Flag[0] == '1') ? 1 : 0;
	}
	return GMoverDebugLog;
}

void AActor::physMovingBrush( FLOAT DeltaTime )
{
	guard(physMovingBrush);
	if( IsA(AMover::StaticClass()) )
	{
		AMover* Mover  = (AMover*)this;
		INT KeyNum     = Clamp( (INT)Mover->KeyNum, (INT)0, (INT)ARRAY_COUNT(Mover->KeyPos) );
		while( Mover->bInterpolating && DeltaTime>0.0f )
		{
			bool bFell = false;
			if( bCollideWorld && Mover->Region.Zone && DeltaTime != 0.0f )
			{
				// Apply gravity as well.
				FVector FallVel = Mover->Region.Zone->ZoneGravity * (Mover->Velocity | Mover->Region.Zone->ZoneGravity) / Mover->Region.Zone->ZoneGravity.SizeSquared();
				FVector Fall = FallVel * DeltaTime + Mover->Region.Zone->ZoneGravity * (DeltaTime*DeltaTime*0.5f);
				Mover->Velocity += Mover->Region.Zone->ZoneGravity * DeltaTime;
				FCheckResult Hit(1.0f);
				FVector OldLoc = Location;
				GetLevel()->MoveActor( Mover, Fall, Mover->Rotation, Hit );
				if( Hit.Time > 0.0f )
				{
					Mover->KeyPos[KeyNum] += Location - OldLoc;
					Mover->OldPos += Location - OldLoc;
					bFell = true;
				}
				else
					FindBase();
			}

			// We are moving.
			FLOAT NewAlpha = Mover->PhysAlpha + DeltaTime * Mover->PhysRate;
			if( NewAlpha > 1.0f )
			{
				if( Mover->PhysAlpha < 1.0f )
					DeltaTime *= (NewAlpha - 1.0f) / (NewAlpha - Mover->PhysAlpha);
				else DeltaTime = 0.0f;
				NewAlpha   = 1.0f;
			}
			else DeltaTime = 0.0f;

			// Compute alpha.
			FLOAT RenderAlpha;

			// set correct MoverGlideType (just for last or first frame)
			unsigned char MoverGlideType = Mover->MoverGlideType; 
			if(MoverGlideType == MV_SpringByTime)
			{
 				if((Mover->PrevKeyNum < Mover->KeyNum) && (Mover->KeyNum != Mover->NumKeys - 1))
  					MoverGlideType = MV_MoveByTime;

				if((Mover->PrevKeyNum > Mover->KeyNum) && (Mover->KeyNum != 0))
  					MoverGlideType = MV_MoveByTime;
			}

 			if( MoverGlideType == MV_GlideByTime )
			{
				// Make alpha time-smooth and time-continuous.
				// f(0)=0, f(1)=1, f'(0)=f'(1)=0.
				RenderAlpha = 3.0f*NewAlpha*NewAlpha - 2.0f*NewAlpha*NewAlpha*NewAlpha;
			}
			else if( MoverGlideType == MV_SpringByTime )
			{
				float fStartSpring		= (Mover->MoveTime - Mover->MoverSpringTime) / Mover->MoveTime;
 				float fMaxAmplitude		= Mover->MoverMaxAmplitude;
				int	  iMoverFluctuations= (int)Mover->MoverFluctuations;

				// set reasonable iMoverFluctuations, if level editor made a zero
				if(iMoverFluctuations == 0)	
					iMoverFluctuations = 1;

				// set reasonable fStartSpring, if level editor made a mistake
				if((fStartSpring <= 0.0) || (fStartSpring >= 1.0))
					fStartSpring = 0.75;

				// set reasonable fMaxAmplitude, if level editor made a mistake
				if((fMaxAmplitude <= 0.0) || (fMaxAmplitude >= 1.0))
					fMaxAmplitude = 0.1;

				RenderAlpha = 1.0;	// just for initialization
				if(NewAlpha <= fStartSpring)
				{
					RenderAlpha = NewAlpha / fStartSpring;
				}
				else
				{
					float delta		= (1.0 - fStartSpring) / (2.0 * iMoverFluctuations);
					float current, previous, next, coeff, currAmpl;

					for (int k = 0; k < iMoverFluctuations; k++)
					{
						current		= fStartSpring + delta * (2.0 * k + 1.0);
						previous	= current - delta;
						next		= current + delta;
						currAmpl	= fMaxAmplitude / (1 << k);
						coeff		= currAmpl / delta;

						if( (NewAlpha > previous) && (NewAlpha <= current) )
						{
							RenderAlpha = -coeff * NewAlpha + 1.0 - currAmpl + coeff * current;
							break;
						}
						else if( (NewAlpha > current ) && (NewAlpha <= next) )
						{
							RenderAlpha =  coeff * NewAlpha + 1.0 - currAmpl - coeff * current;
							break;
						}
					}
				}
			}
			else RenderAlpha = NewAlpha;

			// Move.
			FCheckResult Hit(1.0f);
			if( GetLevel()->MoveActor
			(
				Mover,
				Mover->OldPos + ((Mover->BasePos + Mover->KeyPos[KeyNum]) - Mover->OldPos) * RenderAlpha - Mover->Location,
				Mover->OldRot + ((Mover->BaseRot + Mover->KeyRot[KeyNum]) - Mover->OldRot) * RenderAlpha,
				Hit
			) )
			{
				// Moved some amount.
				Mover->PhysAlpha += Hit.Time * (NewAlpha - Mover->PhysAlpha);
			}

			if( !bFell )
			{
				if( Hit.Time < 1.f )
				{
					// Hit something. Done now.
					Mover->bInterpolating = 0;
					Mover->HitPosition	= Hit.Location;
					Mover->HitNormal	= Hit.Normal;
					if( MoverDebugEnabled() )
						debugf( NAME_Log, TEXT("MoverFreeze: %s froze alpha=%f key %i->%i hit %s"), Mover->GetName(), Mover->PhysAlpha, Mover->PrevKeyNum, Mover->KeyNum, Hit.Actor ? Hit.Actor->GetName() : TEXT("world") );
				}
				else if( Mover->PhysAlpha >= 1.0f )
				{
					// Just finished moving.
					Mover->bInterpolating = 0;
					Mover->eventKeyFrameReached();
				}
			}
		}
	}
	unguard;
}

//
// Initialize execution.
//
void AActor::InitExecution()
{
	guard(AActor::InitExecution);

	UObject::InitExecution();

	check(GetStateFrame());
	check(GetStateFrame()->Object==this);
	check(GetLevel()!=NULL);
	check(GetLevel()->Actors(0)!=NULL);
	check(GetLevel()->Actors(0)==Level);
	check(Level!=NULL);
	HP2ActorTransitionPostInitExecution( this );

	unguardobj;
}

/*-----------------------------------------------------------------------------
	Natives.
-----------------------------------------------------------------------------*/

//////////////////////
// Console Commands //
//////////////////////

void AActor::execConsoleCommand( FFrame& Stack, RESULT_DECL )
{
	guard(UObject::execConsoleCommand);

	P_GET_STR(Command);
	P_FINISH;

	FStringOutputDevice StrOut;
	GetLevel()->Engine->Exec( *Command, StrOut );
	*(FString*)Result = *StrOut;

	unguard;
}

/////////////////////////////
// Log and error functions //
/////////////////////////////

void AActor::execError( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execError);

	P_GET_STR(S);
	P_FINISH;

	Stack.Log( *S );
	GetLevel()->DestroyActor( this );

	unguardexecSlow;
}

//////////////////////////
// Clientside functions //
//////////////////////////

void APlayerPawn::execClientTravel( FFrame& Stack, RESULT_DECL )
{
	guardSlow(APlayerPawn::execClientTravel);

	P_GET_STR(URL);
	P_GET_BYTE(TravelType);
	P_GET_UBOOL(bItems);
	P_FINISH;

	if( Player )
	{
		// Warn the client.
		eventPreClientTravel();

		// Do the travel.
		GetLevel()->Engine->SetClientTravel( Player, *URL, bItems, (ETravelType)TravelType );
	}

	unguardexecSlow;
}

void APlayerPawn::execGetPlayerNetworkAddress( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execGetPlayerNetworkAddress);
	P_FINISH;

	if( Player && Player->IsA(UNetConnection::StaticClass()) )
		*(FString*)Result = Cast<UNetConnection>(Player)->LowLevelGetRemoteAddress();
	else
		*(FString*)Result = TEXT("");
	unguard;
}

void APlayerPawn::execCopyToClipboard( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execCopyToClipboard);
	P_GET_STR(Text);
	P_FINISH;
	appClipboardCopy(*Text);
	unguard;
}

void APlayerPawn::execPasteFromClipboard( FFrame& Stack, RESULT_DECL )
{
	guard(APlayerPawn::execCopyToClipboard);
	P_GET_STR(Text);
	P_FINISH;
	*(FString*)Result = appClipboardPaste();
	unguard;
}

void ALevelInfo::execGetLocalURL( FFrame& Stack, RESULT_DECL )
{
	guardSlow(ALevelInfo::execGetLocalURL);

	P_FINISH;

	*(FString*)Result = GetLevel()->URL.String();

	unguardexecSlow;
}

void ALevelInfo::execGetAddressURL( FFrame& Stack, RESULT_DECL )
{
	guardSlow(ALevelInfo::execGetAddressURL);

	P_FINISH;

	*(FString*)Result = FString::Printf( TEXT("%s:%i"), *GetLevel()->URL.Host, GetLevel()->URL.Port );

	unguardexecSlow;
}

///////////////////////////
// Client-side functions //
///////////////////////////

void APawn::execClientHearSound( FFrame& Stack, RESULT_DECL )
{
	guard(APawn::execClientHearSound);

	P_GET_OBJECT(AActor,Actor);
	P_GET_INT(Id);
	P_GET_OBJECT(USound,Sound);
	P_GET_VECTOR(SoundLocation);
	P_GET_VECTOR(Parameters);
	P_GET_UBOOL(Disable3D);
	P_GET_UBOOL(Loop);
	P_FINISH;

	FLOAT Volume = 0.01f * Parameters.X;	// TG ALPHA - why the 0.01 factor???
	FLOAT Radius = Parameters.Y;
	FLOAT Pitch  = 0.01f * Parameters.Z;
	if
	(	IsA(APlayerPawn::StaticClass()) 
	&&	((APlayerPawn*)this)->Player
	&&	((APlayerPawn*)this)->Player->IsA(UViewport::StaticClass())
	&&	GetLevel()->Engine->Audio )
	{
		if( Actor && Actor->bDeleteMe )
			Actor = NULL;

		// First person sound attenuation hack.
		INT Flags = Sound->CoreFlags;
		
		if (Disable3D || (Actor && ((Actor == ((APlayerPawn*)this)->Player->Actor) || Actor->IsOwnedBy(((APlayerPawn*)this)->Player->Actor)) ))
			Flags |= SF_No3D;

		if (Loop)
			Flags |= SF_Looping;

		GetLevel()->Engine->Audio->PlaySound( Actor, Id, Sound, SoundLocation, Volume, Radius ? Radius : GAudioDefaultRadius, Pitch, Flags, 0.f );
	}
	unguardexec;
}

////////////////////////////////
// Latent function initiators //
////////////////////////////////

void AActor::execSleep( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execSleep);

	P_GET_FLOAT(Seconds);
	P_FINISH;

	GetStateFrame()->LatentAction = EPOLL_Sleep;
	LatentFloat  = Seconds;

	unguardexecSlow;
}

void AActor::execFinishAnim( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execFinishAnim);

	P_GET_NAME(Bone);
	P_FINISH;

	AActor* Chan = this;
	if( Bone != NAME_None )
	{
		// Search for channel.
		USkeletalMesh* SMesh = Cast<USkeletalMesh>(Mesh);
		if( SMesh )
		{
			INT BoneStart = SMesh->BoneIndex( Bone );
			for_array( i, AuxAnims )
			{
				if( AuxAnims(i)->AnimBone == BoneStart )
				{
					Chan = AuxAnims(i);
					break;
				}
			}
		}
		if( Chan == this )
			// No channel found.
			return;
	}

	// If we are looping, finish at the next sequence end.
	if( Chan->bAnimLoop )
	{
		Chan->bAnimLoop     = 0;
		Chan->bAnimFinished = 0;
	}

	// If animation is playing, wait for it to finish.
	if( Chan->IsAnimating() && Chan->AnimFrame<Chan->AnimLast )
		Chan->GetStateFrame()->LatentAction = EPOLL_FinishAnim;

	unguardexecSlow;
}

void AActor::execFinishInterpolation( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execFinishInterpolation);

	P_FINISH;

	GetStateFrame()->LatentAction = EPOLL_FinishInterpolation;

	unguardexecSlow;
}

///////////////////////////
// Slow function pollers //
///////////////////////////

void AActor::execPollSleep( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execPollSleep);

	FLOAT DeltaSeconds = *(FLOAT*)Result;
	if( (LatentFloat-=DeltaSeconds) < 0.5 * DeltaSeconds )
	{
		// Awaken.
		GetStateFrame()->LatentAction = 0;
	}
	unguardexecSlow;
}
IMPLEMENT_FUNCTION( AActor, EPOLL_Sleep, execPollSleep );

void AActor::execPollFinishAnim( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execPollFinishAnim);

	if( bAnimFinished )
		GetStateFrame()->LatentAction = 0;

	unguardexecSlow;
}
IMPLEMENT_FUNCTION( AActor, EPOLL_FinishAnim, execPollFinishAnim );

void AActor::execPollFinishInterpolation( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execPollFinishInterpolation);

	if( !bInterpolating )
		GetStateFrame()->LatentAction = 0;

	unguardexecSlow;
}
IMPLEMENT_FUNCTION( AActor, EPOLL_FinishInterpolation, execPollFinishInterpolation );

/////////////////////////
// Animation functions //
/////////////////////////

IMPLEMENT_CLASS(AAnimChannel);

static inline bool IsSubset( const USkeletalMesh* SMesh, int Bone1, int Bone2 )
{
	// Return whether Bone1's range is a subset of Bone2's, for SMesh.
	return Bone1 >= Bone2 && 
		   Bone1 < Bone2 + SMesh->RefSkeleton(Bone2).NumChildren;
}

// Needed to avoid internal compiler error in VC 6.
#pragma optimize("g", off)

AActor* AActor::CreateAnimChannel( UClass* NewClass, EAnimType Type, FName RootBone, bool bTransient, bool bNotReplaceable )
{
	guard(AActor::CreateAnimChannel);

	USkeletalMesh* SMesh = Cast<USkeletalMesh>(Mesh);
	if( SMesh && RootBone != NAME_None )
	{
		INT BoneStart = SMesh->BoneIndex( RootBone );
		if( BoneStart < 0 )
			return NULL;

		// If bone range is identical to existing channel, replace it.
		for_array( i, AuxAnims )
		{
			AActor* Chan = AuxAnims(i);
			if( Chan->AnimBone == BoneStart && 
				Chan->bAnimTransient == (BITFIELD)bTransient && 
				Chan->bAnimNotReplaceable == (BITFIELD)bNotReplaceable )
				return Chan;
		}

		// Add aux anim channel, at actor location, without collision checking.
		AActor* Chan = GetLevel()->SpawnActor( NewClass, NAME_None, this, NULL, Location, Rotation, NULL, true );
		if (Chan)
		{
			// Make it invisible.
			Chan->Mesh			= Mesh;
			Chan->SkelAnim		= SkelAnim;

			// Set channel properties.
			Chan->AnimBone = BoneStart;
			Chan->bAnimTransient = bTransient;
			Chan->bAnimNotReplaceable = bNotReplaceable;

			if( Type == AT_Combine )
			{
				// Insert it, leaving all others.
				AuxAnims.Insert( 0 );
				AuxAnims(0) = Chan;
			}
			else
			{
				// Append it, and delete subset channels.
				for( INT i=AuxAnims.Num()-1; i>=0; i-- )
				{
					if( IsSubset( SMesh, AuxAnims(i)->AnimBone, BoneStart ) && !AuxAnims(i)->bAnimNotReplaceable )
					{
						GetLevel()->DestroyActor( AuxAnims(i) );
						AuxAnims.Remove( i );
					}
				}

				AuxAnims.AddItem( Chan );
			}
			return Chan;
		}
	}
	return NULL;
	unguard;
}

void AActor::execCreateAnimChannel( FFrame& Stack, RESULT_DECL )
{
	P_GET_OBJECT(UClass,NewClass);
	P_GET_STRUCT(EAnimType,AnimType);
	P_GET_NAME(Bone);
	P_GET_UBOOL_OPTX(bTransient, false);
	P_GET_UBOOL_OPTX(bNotReplaceable, false);
	P_FINISH;

	*(AActor**)Result = CreateAnimChannel( NewClass, AnimType, Bone, !!bTransient, !!bNotReplaceable );
}

UBOOL AActor::PlayAnim( FName SequenceName, bool bLoop, float PlayAnimRate, float TweenTime, float MinRate, EAnimType Type, FName RootBone )
{
	guardSlow(AActor::PlayAnim);

	// Set animation.
	if( Mesh )
	{
		USkeletalMesh* SMesh = Cast<USkeletalMesh>(Mesh);
		if( RootBone == TEXT("Move") )
		{
			RootBone = NAME_None;
			bAnimMove = true;
		}
		else
			bAnimMove = false;

		if( SMesh && RootBone != NAME_None )
		{
			if( SequenceName != NAME_None )
			{
				// Create a transient anim channel.
				LoadObject<UClass>( StaticClass()->GetOuter(), TEXT("AnimChannel"), NULL, 0, NULL );
				AActor* Chan = CreateAnimChannel( AAnimChannel::StaticClass(), Type, RootBone, true, false );
				if (Chan)
				{
					// Dispatch anim call to aux.
					return Chan->PlayAnim( SequenceName, bLoop, PlayAnimRate, TweenTime, MinRate );
				}
			}
			else
			{
				// Remove channels in bone subset.
				INT BoneStart = SMesh->BoneIndex( RootBone );
				if( BoneStart < 0 )
					return false;
				for( INT i=AuxAnims.Num()-1; i>=0; i-- )
				{
					if( IsSubset( SMesh, AuxAnims(i)->AnimBone, BoneStart ) )
					{
						if (!AuxAnims(i)->bAnimNotReplaceable)
						{
							if( AuxAnims(i)->bAnimTransient )
							{
								GetLevel()->DestroyActor( AuxAnims(i) );
								AuxAnims.Remove(i);
							}
							else
							{
								AuxAnims(i)->AnimSequence = NAME_None;
							}
						}
					}
				}
				return true;
			}
		}

		// Animation on whole skeleton.

		if( Type == AT_Replace )
		{
			// Remove aux anims.
			for( INT i=AuxAnims.Num()-1; i>=0; i-- )
			{
				if (!AuxAnims(i)->bAnimNotReplaceable)
				{
					if( AuxAnims(i)->bAnimTransient )
					{
						GetLevel()->DestroyActor( AuxAnims(i) );
						AuxAnims.Remove(i);
					}
					else
					{
						AuxAnims(i)->AnimSequence = NAME_None;
					}
				}
			}
		}

		const FMeshAnimSeq* Seq = GetAnim( SequenceName );		
		if( Seq || SequenceName == NAME_None )
		{
			if( Seq )
			{
				if ( (AnimSequence == SequenceName) && bLoop && bAnimLoop && IsAnimating() )
				{
					AnimRate      = PlayAnimRate * Seq->Rate / Seq->NumFrames;
					bAnimFinished = 0;
					AnimMinRate   = MinRate!=0.0f ? MinRate * (Seq->Rate / Seq->NumFrames) : 0.0f;
					FPlane OldSimAnim = SimAnim;
					SimAnim.Y = 5000 * AnimRate;
					SimAnim.W = -10000 * (1.0f - 1.0f / Seq->NumFrames);
					if ( OldSimAnim == SimAnim )
						SimAnim.W = SimAnim.W + 1;
					return true;
				}

				AnimRate      = PlayAnimRate * Seq->Rate / Seq->NumFrames;
				AnimLast      = 1.0f - 1.0f / Seq->NumFrames;
				AnimMinRate   = MinRate!=0.0f ? MinRate * (Seq->Rate / Seq->NumFrames) : 0.0f;
				bAnimNotify   = Seq->Notifys.Num()!=0;
				bAnimFinished = 0;
				bAnimLoop     = bLoop;
			}
			else
			{
				AnimLast = 0.0f;
				bAnimLoop = 0;
			}

			AnimSequence  = SequenceName;
			bAnimFinished = 0;
			AnimFrame     = 0.0f;
			TweenAlpha    = 0.0f;
			if( AnimLast == 0.0f )
			{
				// Static animation.
				AnimMinRate = 0.0f;
				AnimRate = 0.0f;
				bAnimNotify   = 0;
			}

			// Tween time.
			if( TweenTime > 0.0f )
				TweenRate = 1.0f / TweenTime;
			else if( TweenTime < 0.0f )
				// Auto-compute tween time. To do.
				TweenRate = 2.0f;
			else
			{
				TweenRate = 0.0f;
				TweenAlpha = 1.0f;
			}

			// Update replication vars.
			FPlane OldSimAnim = SimAnim;
			SimAnim.X = 10000 * AnimFrame;
			SimAnim.Y = 5000 * AnimRate;
			if ( SimAnim.Y > 32767 )
				SimAnim.Y = 32767;
			SimAnim.Z = 1000 * TweenRate;
			SimAnim.W = -10000 * AnimLast;
			if( !bLoop )
			{
				if ( OldSimAnim == SimAnim )
					SimAnim.W = SimAnim.W + 1;
			}
			//debugf("%s LoopAnim %f %f %f %f", GetName(), SimAnim.X, SimAnim.Y, SimAnim.Z, SimAnim.W);
			return true;
		}
		else GLog->Logf( TEXT("PlayAnim: Sequence '%s' not found in Mesh '%s'"), *SequenceName, Mesh->GetName() );
	} else GLog->Logf( TEXT("PlayAnim: No mesh in Object '%s' for Sequence '%s'"), GetName(), *SequenceName );
	return false;
	unguardexecSlow;
}

#pragma optimize("", on)

void AActor::execPlayAnim( FFrame& Stack, RESULT_DECL )
{
	P_GET_NAME(SequenceName);
	P_GET_FLOAT_OPTX(PlayAnimRate,1.0f);
	P_GET_FLOAT_OPTX(TweenTime,-1.0f);
	P_GET_STRUCT_OPTX(EAnimType,AnimType,AT_Replace);
	P_GET_NAME(Bone);
	P_FINISH;

	PlayAnim( SequenceName, false, PlayAnimRate, TweenTime, 0.f, AnimType, Bone );
}

void AActor::execLoopAnim( FFrame& Stack, RESULT_DECL )
{
	P_GET_NAME(SequenceName);
	P_GET_FLOAT_OPTX(PlayAnimRate,1.0f);
	P_GET_FLOAT_OPTX(TweenTime,-1.0f);
	P_GET_FLOAT_OPTX(MinRate,0.0f);
	P_GET_STRUCT_OPTX(EAnimType,AnimType,AT_Replace);
	P_GET_NAME(Bone);
	P_FINISH;

	PlayAnim( SequenceName, true, PlayAnimRate, TweenTime, MinRate, AnimType, Bone );
}

void AActor::execTweenAnim( FFrame& Stack, RESULT_DECL )
{
	P_GET_NAME(SequenceName);
	P_GET_FLOAT(TweenTime);
	P_FINISH;

	PlayAnim( SequenceName, false, 0.0f, TweenTime );
}

void AActor::execIsAnimating( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execIsAnimating);

	P_GET_NAME(Bone);
	P_FINISH;

	if( Bone != NAME_None )
	{
		// Search for channel.
		USkeletalMesh* SMesh = Cast<USkeletalMesh>(Mesh);
		if( SMesh )
		{
			INT BoneStart = SMesh->BoneIndex( Bone );
			for_array( i, AuxAnims )
			{
				if( AuxAnims(i)->AnimBone == BoneStart )
				{
					*(DWORD*)Result = AuxAnims(i)->IsAnimating();
					return;
				}
			}
		}

		// None found.
		*(DWORD*)Result = false;
	}
	else
		// Base animation.
		*(DWORD*)Result = IsAnimating();

	unguardexecSlow;
}

void AActor::execGetAnimGroup( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execGetAnimGroup);

	P_GET_NAME(SequenceName);
	P_FINISH;

	// Return the animation group.
	*(FName*)Result = NAME_None;
	if( Mesh )
	{
		const FMeshAnimSeq* Seq = GetAnim( SequenceName );
		if( Seq )
		{
			*(FName*)Result = Seq->Group;
		}
		else Stack.Logf( TEXT("GetAnimGroup: Sequence '%s' not found in Mesh '%s'"), *SequenceName, Mesh->GetName() );
	} else Stack.Logf( TEXT("GetAnimGroup: No mesh") );

	unguardexecSlow;
}

void AActor::execHasAnim( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execHasAnim);

	P_GET_NAME(SequenceName);
	P_FINISH;

	// Check for a certain anim sequence.
	if( Mesh )
	{
		const FMeshAnimSeq* Seq = GetAnim( SequenceName );
		if( Seq )
		{
			*(DWORD*)Result = 1;
		} else
			*(DWORD*)Result = 0;
	} else Stack.Logf( TEXT("HasAnim: No mesh") );
	unguardexecSlow;
}

void AActor::execBoneNumber( FFrame& Stack, RESULT_DECL )
{
	P_GET_NAME(BoneName);
	P_FINISH;

	// Check for a certain anim sequence.
	USkeletalMesh* SMesh = Cast<USkeletalMesh>(Mesh);
	if( SMesh )
		*(DWORD*)Result = SMesh->BoneIndex(BoneName);
	else
		*(DWORD*)Result = 0;
}

void AActor::execBoneName( FFrame& Stack, RESULT_DECL )
{
	P_GET_INT(BoneNumber);
	P_FINISH;

	// Check for a certain anim sequence.
	USkeletalMesh* SMesh = Cast<USkeletalMesh>(Mesh);
	if( SMesh )
		*(FName*)Result = SMesh->BoneName(BoneNumber);
	else
		*(FName*)Result = NAME_None;
}

void AActor::execBonePos( FFrame& Stack, RESULT_DECL )
{
	P_GET_NAME(BoneName);
	P_FINISH;

	// Check for a certain anim sequence.
	USkeletalMesh* SMesh = Cast<USkeletalMesh>(Mesh);
	if( SMesh )
		*(FVector*)Result = SMesh->GetBoneCoords( this, SMesh->BoneIndex(BoneName) ).Origin;
	else
		*(FVector*)Result = Location;
}

void AActor::execBoneRot( FFrame& Stack, RESULT_DECL )
{
	P_GET_NAME(BoneName);
	P_FINISH;

	// Check for a certain anim sequence.
	USkeletalMesh* SMesh = Cast<USkeletalMesh>(Mesh);
	if( SMesh )
		*(FRotator*)Result = SMesh->GetBoneCoords( this, SMesh->BoneIndex(BoneName) ).OrthoRotation();
	else
		*(FRotator*)Result = Rotation;
}

//OrthoRotation()

void AActor::execLinkSkelAnim( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execLinkSkelAnim);
	//native final function LinkSkelAnim( Animation Anim );
	P_GET_OBJECT(UAnimation,Anim);
	//P_GET_INT_OPTX(Slot,0);
	P_FINISH;	
	if (Anim)
	{
		SkelAnim = Anim;
	}
	else
	{
		SkelAnim = NULL;
	}
	unguardexecSlow;
}

///////////////
// Collision //
///////////////

void AActor::execSetCollision( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execSetCollision);

	P_GET_UBOOL_OPTX(NewCollideActors,bCollideActors);
	P_GET_UBOOL_OPTX(NewBlockActors,  bBlockActors  );
	P_GET_UBOOL_OPTX(NewBlockPlayers, bBlockPlayers );
	P_FINISH;

	SetCollision( NewCollideActors, NewBlockActors, NewBlockPlayers );

	unguardexecSlow;
}

void AActor::execSetCollisionSize( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execSetCollisionSize);

	P_GET_FLOAT(NewRadius);
	P_GET_FLOAT(NewHeight);
	P_GET_FLOAT_OPTX(NewWidth, CollisionWidth);
	P_FINISH;

	CollisionWidth = NewWidth;
	SetCollisionSize( NewRadius, NewHeight );

	// Return boolean success or failure.
	*(DWORD*)Result = 1;

	unguardexecSlow;
}

void AActor::execGetWorldCollisionBox( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execGetCollisionBox);

	P_GET_UBOOL(bVisual);
	P_FINISH;

	UPrimitive* Prim;
	if( bVisual )
	{
		// Override GetPrimitive and use shape.
		if( Mesh ) 
			Prim = Mesh;
		else if ( Brush  ) 
			Prim = Brush;
		else
			Prim = GetPrimitive();
	}
	else
		Prim = GetPrimitive();

	*(FBox*)Result = Prim->GetCollisionBoundingBox( this, true );

	unguardexecSlow;
}

void AActor::execGetRenderExtent( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execGetRenderExtent);

	P_FINISH;

	// Hackus.
	FBox Box(0);
	if( Cast<USkeletalMesh>(Mesh) )
		Box = ((USkeletalMesh*)Mesh)->BoundingBoxSum;
	else
	{
		UPrimitive* Prim;
		if( Mesh )			Prim = Mesh;
		else if( Brush  )	Prim = Brush;
		else				Prim = GetPrimitive();

		Box = Prim->GetCollisionBoundingBox( this, false );
	}
	*(FVector*)Result = Box.Max - Box.Min;

	unguardexecSlow;
}

void AActor::execSetBase( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execSetFloor);

	P_GET_OBJECT(AActor,NewBase);
	P_FINISH;

	SetBase( NewBase );

	unguardSlow;
}

///////////
// Audio //
///////////
void AActor::CheckHearSound(APawn* Hearer, INT Id, USound* Sound, FVector Parameters, FLOAT RadiusSquared, UBOOL Disable3D, UBOOL Loop)
{
	guardSlow(AActor::CheckHearSound);

	FVector HearSource;
	if ( Hearer->IsA(APlayerPawn::StaticClass()) && ((APlayerPawn *)Hearer)->ViewTarget )
		HearSource = ((APlayerPawn *)Hearer)->ViewTarget->Location;
	else
		HearSource = Hearer->Location;

	FLOAT NewRadiusSquared = RadiusSquared/1.3f;
	FLOAT DistSq = (HearSource-Location).SizeSquared();
	if( DistSq < NewRadiusSquared )
	{
		if ( !GetLevel()->Model->FastLineCheck(HearSource,Location) )
		{
			// if no line of sight, reduce radius and volume
			if ( Instigator != Hearer )
				NewRadiusSquared *= 0.6f;

			Parameters.X *= 0.35f;
			if ( DistSq > NewRadiusSquared )
				return;
		}
		Hearer->eventClientHearSound( this, Id, Sound, Location, Parameters, Disable3D, Loop );
	}

	unguardSlow;
}

void AActor::execDemoPlaySound( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execDemoPlaySound);

	// Get parameters.
	P_GET_OBJECT(USound,Sound);
	P_GET_BYTE_OPTX(Slot,SLOT_Misc);
	P_GET_FLOAT_OPTX(Volume,TransientSoundVolume);
	P_GET_UBOOL_OPTX(bNoOverride, 0);
	P_GET_FLOAT_OPTX(Radius,TransientSoundRadius);
	P_GET_FLOAT_OPTX(Pitch,(TransientSoundPitch/64));
	P_GET_UBOOL_OPTX(Disable3D, false);
	P_GET_UBOOL_OPTX(Loop, false);
	P_FINISH;

	if( !Sound )
		return;

	// Play the sound locally

	INT Id = GetIndex()*16 + Slot*2 + bNoOverride;
	FLOAT RadiusSquared = Square( Radius ? Radius : 1600.f );
	FVector Parameters = FVector(100 * Volume, Radius, 100 * Pitch);

	UClient* Client = GetLevel()->Engine->Client;
	if( Client )
	{
		for( INT i=0; i<Client->Viewports.Num(); i++ )
		{
			APlayerPawn* Hearer = Client->Viewports(i)->Actor;
			if( Hearer && Hearer->GetLevel()==GetLevel() )
				CheckHearSound(Hearer, Id, Sound, Parameters,RadiusSquared, Disable3D, Loop);
		}
	}
	unguardexecSlow;
}

void AActor::execPlayMusic( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execPlayMusic);
	P_GET_STR(Song);
	P_GET_FLOAT(FadeInTime);
	P_FINISH;

	INT SongHandle = 0;

	if( GetLevel()->Engine->Audio )
		SongHandle = GetLevel()->Engine->Audio->PlayMusic( Song, FadeInTime );

	*(INT*)Result  = SongHandle;

	unguardexecSlow;
}

void AActor::execStopMusic( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execStopMusic);
	P_GET_INT( SongHandle );
	P_GET_FLOAT( FadeOutTime );
	P_FINISH;	

	if( GetLevel()->Engine->Audio )
		GetLevel()->Engine->Audio->StopMusic( SongHandle, FadeOutTime );

	unguardexecSlow;
}

void AActor::execStopAllMusic( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execStopAllMusic);
	P_GET_FLOAT( FadeOutTime );
	P_FINISH;	

	if( GetLevel()->Engine->Audio )
		GetLevel()->Engine->Audio->StopAllMusic( FadeOutTime );

	unguardexecSlow;
}

#pragma DISABLE_OPTIMIZATION
void AActor::execPlaySound( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execPlaySound);

	// Get parameters.
	P_GET_OBJECT(USound,Sound);
	P_GET_BYTE_OPTX(Slot,SLOT_Misc);
	P_GET_FLOAT_OPTX(Volume,TransientSoundVolume);
	P_GET_UBOOL_OPTX(bNoOverride, 0);
	P_GET_FLOAT_OPTX(Radius,TransientSoundRadius);
	P_GET_FLOAT_OPTX(Pitch,(TransientSoundPitch/64));
	P_GET_UBOOL_OPTX(Disable3D, false);
	P_GET_UBOOL_OPTX(Loop, false);
	P_FINISH;

	if( !Sound )
		return;

	// Server-side demo needs a call to execDemoPlaySound for the DemoRecSpectator
	if(		GetLevel() && GetLevel()->DemoRecDriver
		&&	!GetLevel()->DemoRecDriver->ServerConnection
		&&	GetLevel()->GetLevelInfo()->NetMode != NM_Client )
		eventDemoPlaySound(Sound, Slot, Volume, bNoOverride, Radius, Pitch, Disable3D, Loop);

	INT Id = GetIndex()*16 + Slot*2 + bNoOverride;
	FLOAT RadiusSquared = Square( Radius ? Radius : 1600.f );
	FVector Parameters = FVector(100 * Volume, Radius, 100 * Pitch);

	// See if the function is simulated.
	UFunction* Caller = Cast<UFunction>( Stack.Node );
	if( (GetLevel()->GetLevelInfo()->NetMode == NM_Client) || (Caller && (Caller->FunctionFlags & FUNC_Simulated)) )
	{
		// Called from a simulated function, so propagate locally only.
		UClient* Client = GetLevel()->Engine->Client;
		if( Client )
		{
			for( INT i=0; i<Client->Viewports.Num(); i++ )
			{
				APlayerPawn* Hearer = Client->Viewports(i)->Actor;
				if( Hearer && Hearer->GetLevel()==GetLevel() )
					CheckHearSound(Hearer, Id, Sound, Parameters,RadiusSquared, Disable3D, Loop);
			}
		}
	}
	else
	{
		// Propagate to all player actors.
		for( APawn* Hearer=Level->PawnList; Hearer; Hearer=Hearer->nextPawn )
		{
			if( Hearer->bIsPlayer )
				CheckHearSound(Hearer, Id, Sound, Parameters,RadiusSquared, Disable3D, Loop);
		}
	}
	unguardexecSlow;
}
#pragma ENABLE_OPTIMIZATION

void AActor::execPlayOwnedSound( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execPlayOwnedSound);

	// Get parameters.
	P_GET_OBJECT(USound,Sound);
	P_GET_BYTE_OPTX(Slot,SLOT_Misc);
	P_GET_FLOAT_OPTX(Volume,TransientSoundVolume);
	P_GET_UBOOL_OPTX(bNoOverride, 0);
	P_GET_FLOAT_OPTX(Radius,TransientSoundRadius);
	P_GET_FLOAT_OPTX(Pitch,(TransientSoundPitch/64));
	P_GET_UBOOL_OPTX(Disable3D, false);
	P_GET_UBOOL_OPTX(Loop, false);
	P_FINISH;

	if( !Sound )
		return;
	// if we're recording a demo, make a call to execDemoPlaySound()
	if( (GetLevel() && GetLevel()->DemoRecDriver && !GetLevel()->DemoRecDriver->ServerConnection) )
		eventDemoPlaySound(Sound, Slot, Volume, bNoOverride, Radius, Pitch, Disable3D, Loop);

	INT Id = GetIndex()*16 + Slot*2 + bNoOverride;
	FLOAT RadiusSquared = Square( Radius ? Radius : 1600.f );
	FVector Parameters = FVector(100 * Volume, Radius, 100 * Pitch);

	if( GetLevel()->GetLevelInfo()->NetMode == NM_Client )
	{
		UClient* Client = GetLevel()->Engine->Client;
		if( Client )
		{
			for( INT i=0; i<Client->Viewports.Num(); i++ )
			{
				APlayerPawn* Hearer = Client->Viewports(i)->Actor;
				if( Hearer && Hearer->GetLevel()==GetLevel() )
					CheckHearSound(Hearer, Id, Sound, Parameters,RadiusSquared, Disable3D, Loop);
			}
		}
	}
	else
	{
		AActor *RemoteOwner = NULL;
		if( GetLevel()->GetLevelInfo()->NetMode != NM_Standalone )
		{
			if ( IsA(APlayerPawn::StaticClass()) )
			{
				if ( ((APlayerPawn *)this)->Player
					&& !((APlayerPawn*)this)->Player->IsA(UViewport::StaticClass()) )
					RemoteOwner = this;
			}
			else if ( Owner && Owner->IsA(APlayerPawn::StaticClass()) && ((APlayerPawn *)Owner)->Player
					&& !((APlayerPawn*)Owner)->Player->IsA(UViewport::StaticClass()) )
				RemoteOwner = Owner;
		}

		for( APawn* Hearer=Level->PawnList; Hearer; Hearer=Hearer->nextPawn )
		{
			if( Hearer->bIsPlayer && (Hearer != RemoteOwner) )
				CheckHearSound(Hearer, Id, Sound, Parameters,RadiusSquared, Disable3D, Loop);
		}
	}
	unguardexecSlow;
}

void AActor::execGetSoundDuration( FFrame& Stack, RESULT_DECL )
{
	guard(AActor::execGetSoundDuration);

	// Get parameters.
	P_GET_OBJECT(USound,Sound);
	P_FINISH;

	if ( Sound )
		*(FLOAT*)Result = Sound->GetDuration();
	else
		*(FLOAT*)Result = 0;

	unguardexec;
}

void AActor::execModifySound(FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execModifySound);

	P_GET_BYTE(Param);
	P_GET_FLOAT(Value);

	P_GET_OBJECT_OPTX(USound,Sound, NULL);
	P_GET_BYTE_OPTX(Slot,SLOT_Misc);
	P_FINISH;

	if( GetLevel()->Engine->Audio )
	{
		INT Id = GetIndex()*16 + Slot*2;
		*(UBOOL*)Result = GetLevel()->Engine->Audio->ModifySound( this, Id, Sound, Param, Value );
	}
	else
		*(UBOOL*)Result = 0;

	unguardexecSlow;
}

void AActor::execStopSound( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execStopSound);

	P_GET_OBJECT_OPTX(USound,Sound, NULL);
	P_GET_BYTE_OPTX(Slot, SLOT_Misc);
	P_GET_FLOAT_OPTX(FadeOutTime, 0.f);
	P_FINISH;

	INT Id = GetIndex()*16 + Slot*2;

	if( GetLevel()->Engine->Audio )
		GetLevel()->Engine->Audio->StopSound( this, Id, Sound, FadeOutTime);

	unguardexecSlow;
}

//////////////
// Movement //
//////////////

void AActor::execMove( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execMove);

	P_GET_VECTOR(Delta);
	P_FINISH;

	FCheckResult Hit(1.0f);
	*(DWORD*)Result = GetLevel()->MoveActor( this, Delta, Rotation, Hit );

	unguardexecSlow;
}

void AActor::execSetLocation( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execSetLocation);

	P_GET_VECTOR(NewLocation);
	P_FINISH;

	*(DWORD*)Result = GetLevel()->FarMoveActor( this, NewLocation );

	unguardexecSlow;
}

void AActor::execSetRotation( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execSetRotation);

	P_GET_ROTATOR(NewRotation);
	P_FINISH;

	FCheckResult Hit(1.0f);
	*(DWORD*)Result = GetLevel()->MoveActor( this, FVector(0,0,0), NewRotation, Hit );

	unguardexecSlow;
}

///////////////
// Relations //
///////////////

void AActor::execSetOwner( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execSetOwner);

	P_GET_ACTOR(NewOwner);
	P_FINISH;

	SetOwner( NewOwner );

	unguardexecSlow;
}

////////////////

void AActor::execSaveGameExists( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execSaveGameExists);

	//P_GET_ACTOR(NewOwner);
	P_FINISH;

	FString Temp;
	Temp = FString::Printf( TEXT("%s") PATH_SEPARATOR TEXT("Save%i.usa"), *GSys->SaveSlotPath, 9 );

	*(DWORD*)Result = ( GFileManager->FileSize(*Temp) > 0 );

	unguardexecSlow;
}

//////////////////
// Line tracing //
//////////////////

void AActor::execTrace( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execTrace);

	P_GET_VECTOR_REF(HitLocation);
	P_GET_VECTOR_REF(HitNormal);
	P_GET_VECTOR(TraceEnd);
	P_GET_VECTOR_OPTX(TraceStart,Location);
	P_GET_UBOOL_OPTX(bTraceActors,bCollideActors);
	P_GET_VECTOR_OPTX(TraceExtent,FVector(0,0,0));
	P_FINISH;

	// Trace the line.
	FCheckResult Hit(1.0f);
	DWORD TraceFlags;
	if( bTraceActors )
		TraceFlags = TRACE_AllColliding;
	else
		TraceFlags = TRACE_VisBlocking;

	GetLevel()->SingleLineCheck( Hit, this, TraceEnd, TraceStart, TraceFlags, TraceExtent );
	/*if( Hit.Actor && Hit.Item!=INDEX_NONE )
	{
		UModel*  Model = Hit.Actor->IsA(ULevelInfo::StaticClass) ? XLevel->Model : Actor->Model;
		FBspNode& Node = Model->Nodes( Hit.Item );
		FBspSurf& Surf = Model->Surfs( Node.iSurf );
		UTexture* HitTexture = Surf->Texture;
		//do something with HitTexture
	}*/
	*(AActor**)Result = Hit.Actor;
	*HitLocation      = Hit.Location;
	*HitNormal        = Hit.Normal;

	unguardexecSlow;
}

void AActor::execFastTrace( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execTrace);

	P_GET_VECTOR(TraceEnd);
	P_GET_VECTOR_OPTX(TraceStart,Location);
	P_FINISH;

	// Trace the line.
	*(DWORD*)Result = GetLevel()->Model->FastLineCheck(TraceEnd, TraceStart);

	unguardexecSlow;
}

///////////////////////
// Spawn and Destroy //
///////////////////////

void AActor::execSpawn( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execSpawn);

	P_GET_OBJECT(UClass,SpawnClass);
	P_GET_OBJECT_OPTX(AActor,SpawnOwner,NULL); 
	P_GET_NAME_OPTX(SpawnName,NAME_None);
	P_GET_VECTOR_OPTX(SpawnLocation,Location);
	P_GET_ROTATOR_OPTX(SpawnRotation,Rotation);
	P_FINISH;
	HP2CreatureGeneratorTraceClassResolution( SpawnClass, this );
	if( SpawnClass )
		HP2CreatureGeneratorTraceSpawnRequest( SpawnClass, this );

	// Spawn and return actor.
	AActor* Spawned = SpawnClass ? GetLevel()->SpawnActor
	(
		SpawnClass,
		NAME_None,
		SpawnOwner,
		Instigator,
		SpawnLocation,
		SpawnRotation
	) : NULL;
	if( SpawnClass )
	{
		HP2CreatureGeneratorTraceSpawnResult( this, Spawned );
		HP2CreatureGeneratorTraceSpawnPublished( this, Spawned );
	}
	if( Spawned )
		Spawned->Tag = SpawnName;
	*(AActor**)Result = Spawned;

	unguardexecSlow;
}

void AActor::execDestroy( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execDestroy);

	P_FINISH;
	
	*(DWORD*)Result = GetLevel()->DestroyActor( this );

	unguardexecSlow;
}

////////////
// Timing //
////////////

void AActor::execSetTimer( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execSetTimer);

	P_GET_FLOAT(NewTimerRate);
	P_GET_UBOOL(bLoop);
	P_FINISH;

	TimerCounter = 0.0f;
	TimerRate    = NewTimerRate;
	bTimerLoop   = bLoop;

	unguardexecSlow;
}

////////////////
// Warp zones //
////////////////

void AWarpZoneInfo::execWarp( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AWarpZoneInfo::execWarp);

	P_GET_VECTOR_REF(WarpLocation);
	P_GET_VECTOR_REF(WarpVelocity);
	P_GET_ROTATOR_REF(WarpRotation);
	P_FINISH;

	// Perform warping.
	*WarpLocation = (*WarpLocation).TransformPointBy ( WarpCoords.Transpose() );
	*WarpVelocity = (*WarpVelocity).TransformVectorBy( WarpCoords.Transpose() );
	*WarpRotation = (GMath.UnitCoords / *WarpRotation * WarpCoords.Transpose()).OrthoRotation();

	unguardexecSlow;
}

void AWarpZoneInfo::execUnWarp( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AWarpZoneInfo::execUnWarp);

	P_GET_VECTOR_REF(WarpLocation);
	P_GET_VECTOR_REF(WarpVelocity);
	P_GET_ROTATOR_REF(WarpRotation);
	P_FINISH;

	// Perform unwarping.
	*WarpLocation = (*WarpLocation).TransformPointBy ( WarpCoords );
	*WarpVelocity = (*WarpVelocity).TransformVectorBy( WarpCoords );
	*WarpRotation = (GMath.UnitCoords / *WarpRotation * WarpCoords).OrthoRotation();

	unguardexecSlow;
}

/*-----------------------------------------------------------------------------
	Native iterator functions.
-----------------------------------------------------------------------------*/

void AActor::execAllActors( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execAllActors);

	// Get the parms.
	P_GET_OBJECT(UClass,BaseClass);
	P_GET_ACTOR_REF(OutActor);
	P_GET_NAME_OPTX(TagName,NAME_None);
	P_FINISH;

	BaseClass = BaseClass ? BaseClass : AActor::StaticClass();
	INT iActor=0;
	INT iYielded=0;

	PRE_ITERATOR;
		// Fetch next actor in the iteration.
		*OutActor = NULL;
		while( iActor<GetLevel()->Actors.Num() && *OutActor==NULL )
		{
			AActor* TestActor = GetLevel()->Actors(iActor++);
			if(	TestActor && TestActor->IsA(BaseClass) && (TagName==NAME_None || TestActor->Tag==TagName) )
				*OutActor = TestActor;
		}
		if( *OutActor == NULL )
		{
			if( GActorLifecycleObserver )
				GActorLifecycleObserver->OnActorAllActors( this, BaseClass, TagName, NULL, INDEX_NONE, 1 );
			Stack.Code = &Stack.Node->Script(wEndOffset + 1);
			break;
		}
		appRecordPatrolPointIterator( this, BaseClass, TagName, iActor-1, *OutActor, iYielded++ );
		if( GActorLifecycleObserver )
			GActorLifecycleObserver->OnActorAllActors( this, BaseClass, TagName, *OutActor, iActor-1, 0 );
	POST_ITERATOR;

	unguardexecSlow;
}

void AActor::execChildActors( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execChildActors);

	P_GET_OBJECT(UClass,BaseClass);
	P_GET_ACTOR_REF(OutActor);
	P_FINISH;

	BaseClass = BaseClass ? BaseClass : AActor::StaticClass();
	INT iActor=0;

	PRE_ITERATOR;
		// Fetch next actor in the iteration.
		*OutActor = NULL;
		while( iActor<GetLevel()->Actors.Num() && *OutActor==NULL )
		{
			AActor* TestActor = GetLevel()->Actors(iActor++);
			if(	TestActor && TestActor->IsA(BaseClass) && TestActor->IsOwnedBy( this ) )
				*OutActor = TestActor;
		}
		if( *OutActor == NULL )
		{
			Stack.Code = &Stack.Node->Script(wEndOffset + 1);
			break;
		}
	POST_ITERATOR;

	unguardexecSlow;
}

void AActor::execBasedActors( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execBasedActors);

	P_GET_OBJECT(UClass,BaseClass);
	P_GET_ACTOR_REF(OutActor);
	P_FINISH;

	BaseClass = BaseClass ? BaseClass : AActor::StaticClass();
	INT iActor=0;

	PRE_ITERATOR;
		// Fetch next actor in the iteration.
		*OutActor = NULL;
		while( iActor<GetLevel()->Actors.Num() && *OutActor==NULL )
		{
			AActor* TestActor = GetLevel()->Actors(iActor++);
			if(	TestActor && TestActor->IsA(BaseClass) && TestActor->Base==this )
				*OutActor = TestActor;
		}
		if( *OutActor == NULL )
		{
			Stack.Code = &Stack.Node->Script(wEndOffset + 1);
			break;
		}
	POST_ITERATOR;

	unguardexecSlow;
}

void AActor::execTouchingActors( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execTouchingActors);

	P_GET_OBJECT(UClass,BaseClass);
	P_GET_ACTOR_REF(OutActor);
	P_FINISH;

	BaseClass = BaseClass ? BaseClass : AActor::StaticClass();
	INT iTouching=0;

	PRE_ITERATOR;
		// Fetch next actor in the iteration.
		*OutActor = NULL;
		for( iTouching; iTouching<ARRAY_COUNT(Touching) && *OutActor==NULL; iTouching++ )
		{
			AActor* TestActor = Touching[iTouching];
			if(	TestActor && TestActor->IsA(BaseClass) )
				*OutActor = TestActor;
		}
		if( *OutActor == NULL )
		{
			Stack.Code = &Stack.Node->Script(wEndOffset + 1);
			break;
		}
	POST_ITERATOR;

	unguardexecSlow;
}

void AActor::execTraceActors( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execTraceActors);

	P_GET_OBJECT(UClass,BaseClass);
	P_GET_ACTOR_REF(OutActor);
	P_GET_VECTOR_REF(HitLocation);
	P_GET_VECTOR_REF(HitNormal);
	P_GET_VECTOR(End);
	P_GET_VECTOR_OPTX(Start,Location);
	P_GET_VECTOR_OPTX(TraceExtent,FVector(0,0,0));
	P_FINISH;

	FMemMark Mark(GMem);
	BaseClass         = BaseClass ? BaseClass : AActor::StaticClass();
	FCheckResult* Hit = GetLevel()->MultiLineCheck( GMem, End, Start, TraceExtent, 1, Level, 0 );

	PRE_ITERATOR;
		if( Hit )
		{
			*OutActor    = Hit->Actor;
			*HitLocation = Hit->Location;
			*HitNormal   = Hit->Normal;
			Hit          = Hit->GetNext();
		}
		else
		{
			Stack.Code = &Stack.Node->Script(wEndOffset + 1);
			*OutActor = NULL;
			break;
		}
	POST_ITERATOR;
	Mark.Pop();

	unguardexecSlow;
}

void AActor::execRadiusActors( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execRadiusActors);

	P_GET_OBJECT(UClass,BaseClass);
	P_GET_ACTOR_REF(OutActor);
	P_GET_FLOAT(Radius);
	P_GET_VECTOR_OPTX(TraceLocation,Location);
	P_FINISH;

	BaseClass = BaseClass ? BaseClass : AActor::StaticClass();
	INT iActor=0;

	PRE_ITERATOR;
		// Fetch next actor in the iteration.
		*OutActor = NULL;
		while( iActor<GetLevel()->Actors.Num() && *OutActor==NULL )
		{
			AActor* TestActor = GetLevel()->Actors(iActor++);
			if
			(	TestActor
			&&	TestActor->IsA(BaseClass) 
			&&	(TestActor->Location - TraceLocation).SizeSquared() < Square(Radius + TestActor->CollisionRadius) )
				*OutActor = TestActor;
		}
		if( *OutActor == NULL )
		{
			Stack.Code = &Stack.Node->Script(wEndOffset + 1);
			break;
		}
	POST_ITERATOR;

	unguardexecSlow;
}

void AActor::execVisibleActors( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execVisibleActors);

	P_GET_OBJECT(UClass,BaseClass);
	P_GET_ACTOR_REF(OutActor);
	P_GET_FLOAT_OPTX(Radius,0.0f);
	P_GET_VECTOR_OPTX(TraceLocation,Location);
	P_FINISH;

	BaseClass = BaseClass ? BaseClass : AActor::StaticClass();
	INT iActor=0;

	PRE_ITERATOR;
		// Fetch next actor in the iteration.
		*OutActor = NULL;
		while( iActor<GetLevel()->Actors.Num() && *OutActor==NULL )
		{
			AActor* TestActor = GetLevel()->Actors(iActor++);
			if
			(	TestActor
			&& !TestActor->bHidden
			&&	TestActor->IsA(BaseClass)
			&&	(Radius==0.0f || (TestActor->Location-TraceLocation).SizeSquared() < Square(Radius))
			&&	GetLevel()->Model->FastLineCheck(TestActor->Location, TraceLocation) )
				*OutActor = TestActor;
		}
		if( *OutActor == NULL )
		{
			Stack.Code = &Stack.Node->Script(wEndOffset + 1);
			break;
		}
	POST_ITERATOR;

	unguardexecSlow;
}

void AActor::execVisibleCollidingActors( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execVisibleCollidingActors);

	P_GET_OBJECT(UClass,BaseClass);
	P_GET_ACTOR_REF(OutActor);
	P_GET_FLOAT_OPTX(Radius,0.0f);
	P_GET_VECTOR_OPTX(TraceLocation,Location);
	P_GET_UBOOL_OPTX(bIgnoreHidden, 0); 
	P_FINISH;

	Radius = Radius ? Radius : 1000;
	BaseClass = BaseClass ? BaseClass : AActor::StaticClass();
	FMemMark Mark(GMem);
	FCheckResult* Link=GetLevel()->Hash->ActorRadiusCheck( GMem, TraceLocation, Radius, 0 );
	
	PRE_ITERATOR;
		// Fetch next actor in the iteration.
		*OutActor = NULL;
		if ( Link )
		{
			while
			(	Link
			&&	(!Link->Actor
			||	!Link->Actor->IsA(BaseClass) 
			||  (bIgnoreHidden && Link->Actor->bHidden)
			||	!GetLevel()->Model->FastLineCheck(Link->Actor->Location, TraceLocation)) )
				Link=Link->GetNext();

			if ( Link )
			{
				*OutActor = Link->Actor;
				Link=Link->GetNext();
			}
		}
		if ( *OutActor == NULL ) 
		{
			Stack.Code = &Stack.Node->Script(wEndOffset + 1);
			break;
		}
	POST_ITERATOR;

	Mark.Pop();
	unguardexecSlow;
}

void AZoneInfo::execZoneActors( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AZoneInfo::execZoneActors);

	P_GET_OBJECT(UClass,BaseClass);
	P_GET_ACTOR_REF(OutActor);
	P_FINISH;

	BaseClass = BaseClass ? BaseClass : AActor::StaticClass();
	INT iActor=0;

	PRE_ITERATOR;
		// Fetch next actor in the iteration.
		*OutActor = NULL;
		while( iActor<GetLevel()->Actors.Num() && *OutActor==NULL )
		{
			AActor* TestActor = GetLevel()->Actors(iActor++);
			if
			(	TestActor
			&&	TestActor->IsA(BaseClass)
			&&	TestActor->IsInZone(this) )
				*OutActor = TestActor;
		}
		if( *OutActor == NULL )
		{
			Stack.Code = &Stack.Node->Script(wEndOffset + 1);
			break;
		}
	POST_ITERATOR;

	unguardexecSlow;
}

/*-----------------------------------------------------------------------------
	Script processing function.
-----------------------------------------------------------------------------*/

//
// Execute the state code of the actor.
//
void AActor::ProcessState( FLOAT DeltaSeconds )
{
	FStateFrame* StateFrame = GetStateFrame();
	UObject* StateIdentity = StateFrame ? StateFrame->StateNode : NULL;
	const char* ProcessStateOutcome = "skip_no_frame";
	UBOOL ProcessStateDispatched = 0;
	if( StateFrame )
	{
		if( !StateFrame->Code )
			ProcessStateOutcome = "skip_no_code";
		else if( !(Role>=ROLE_Authority || (StateFrame->StateNode->StateFlags & STATE_Simulated)) )
			ProcessStateOutcome = "skip_role";
		else if( IsPendingKill() )
			ProcessStateOutcome = "skip_pending_kill";
		else
		{
			ProcessStateOutcome = "dispatched";
			ProcessStateDispatched = 1;
		}
	}
	HP2GlobalTickTraceProcessState( this, StateIdentity, ProcessStateOutcome );
	if( ProcessStateDispatched )
	{
		UState* OldStateNode = GetStateFrame()->StateNode;
		guard(AActor::ProcessState);
		FAppRandTraceOuterScope RandTrace( this, GetStateFrame()->StateNode );
		if( ++GScriptEntryTag==1 )
			clock(GScriptCycles);

		// If a latent action is in progress, update it.
		if( GetStateFrame()->LatentAction )
			(this->*GNatives[GetStateFrame()->LatentAction])( *GetStateFrame(), (BYTE*)&DeltaSeconds );

		// Execute code.
		INT NumStates=0;
		while( !bDeleteMe && GetStateFrame()->Code && !GetStateFrame()->LatentAction )
		{
			BYTE Buffer[MAX_CONST_SIZE];
			GetStateFrame()->Step( this, Buffer );
			if( GetStateFrame()->StateNode!=OldStateNode )
			{
				OldStateNode = GetStateFrame()->StateNode;
				if( ++NumStates > 4 )
				{
					//GetStateFrame().Logf( "Pause going from %s to %s", xx, yy );
					break;
				}
			}
		}
		if( --GScriptEntryTag==0 )
			unclock(GScriptCycles);
		unguardf(( TEXT("Object %s, Old State %s, New State %s"), GetFullName(), OldStateNode->GetFullName(), GetStateFrame()->StateNode->GetFullName() ));
	}
	if( GActorLifecycleObserver )
		GActorLifecycleObserver->OnActorProcessState( this, DeltaSeconds );
}

//
// Internal RPC calling.
//
static inline void InternalProcessRemoteFunction
(
	AActor*			Actor,
	UNetConnection*	Connection,
	UFunction*		Function,
	void*			Parms,
	FFrame*			Stack,
	UBOOL			IsServer
)
{
	guardSlow(InternalProcessRemoteFunction);
	Actor->GetLevel()->NumRPC++;

	// Make sure this function exists for both parties.
	FClassNetCache* ClassCache = Connection->PackageMap->GetClassNetCache( Actor->GetClass() );
	if( !ClassCache )
		return;
	FFieldNetCache* FieldCache = ClassCache->GetFromField( Function );
	if( !FieldCache )
		return;

	// Get the actor channel.
	UActorChannel* Ch = Connection->ActorChannels.FindRef(Actor);
	if( !Ch )
	{
		if( IsServer )
			Ch = (UActorChannel *)Connection->CreateChannel( CHTYPE_Actor, 1 );
		if( !Ch )
			return;
		if( IsServer )
			Ch->SetChannelActor( Actor );
	}

	// Make sure initial channel-opening replication has taken place.
	if( Ch->OpenPacketId==INDEX_NONE )
	{
		if( !IsServer )
			return;
		Ch->ReplicateActor();
	}

	// Form the RPC preamble.
	FOutBunch Bunch( Ch, 0 );
	//debugf(TEXT("   Call %s"),Function->GetFullName());
	Bunch.WriteInt( FieldCache->FieldNetIndex, ClassCache->GetMaxIndex() );

	// Form the RPC parameters.
	if( Stack )
	{
		appMemzero( Parms, Function->ParmsSize );
		for( TFieldIterator<UProperty> It(Function); It && (It->PropertyFlags & (CPF_Parm|CPF_ReturnParm))==CPF_Parm; ++It )
			Stack->Step( Stack->Object, (BYTE*)Parms + It->Offset );
		checkSlow(*Stack->Code==EX_EndFunctionParms);
	}
	for( TFieldIterator<UProperty> It(Function); It && (It->PropertyFlags & (CPF_Parm|CPF_ReturnParm))==CPF_Parm; ++It )
	{
		if( Connection->PackageMap->ObjectToIndex(*It)!=INDEX_NONE )
		{
			UBOOL Send = 1;
			if( !It->IsA(UBoolProperty::StaticClass()) )
			{
				Send = !It->Matches(Parms,NULL,0);
				Bunch.WriteBit( Send );
			}
			if( Send )
				It->NetSerializeItem( Bunch, Connection->PackageMap, (BYTE*)Parms + It->Offset );
		}
	}

	// Reliability.
	//warning: RPC's might overflow, preventing reliable functions from getting thorough.
	if( Function->FunctionFlags & FUNC_NetReliable )
		Bunch.bReliable = 1;

	// Send the bunch.
	if( !Bunch.IsError() )
		Ch->SendBunch( &Bunch, 1 );
	else
		debugf( NAME_DevNet, TEXT("RPC bunch overflowed") );

	unguardSlow;
}

//
// Return whether a function should be executed remotely.
//
UBOOL AActor::ProcessRemoteFunction( UFunction* Function, void* Parms, FFrame* Stack )
{
	guard(AActor::ProcessRemoteFunction);

	// Quick reject.
	if( (Function->FunctionFlags & FUNC_Static) || bDeleteMe )
		return 0;
	UBOOL Absorb = Role<=ROLE_SimulatedProxy && !(Function->FunctionFlags & FUNC_Simulated);
	if( GetLevel()->DemoRecDriver )
	{
		if( GetLevel()->DemoRecDriver->ServerConnection )
			return Absorb;
		ProcessDemoRecFunction( Function, Parms, Stack );
	}
	if( Level->NetMode==NM_Standalone )
		return 0;
	if( !(Function->FunctionFlags & FUNC_Net) )
		return Absorb;

	// Check if the actor can potentially call remote functions.
	APlayerPawn*    Top              = Cast<APlayerPawn>(GetTopOwner());
	UNetConnection* ClientConnection = NULL;
	if
	(	(Role==ROLE_Authority)
	&&	(Top==NULL || (ClientConnection=Cast<UNetConnection>(Top->Player))==NULL) )
		return Absorb;

	// See if UnrealScript replication condition is met.
	while( Function->GetSuperFunction() )
		Function = Function->GetSuperFunction();
	UBOOL Val=0;
	FFrame( this, Function->GetOwnerClass(), Function->RepOffset, NULL ).Step( this, &Val );
	if( !Val )
		return Absorb;

	// Get the connection.
	UBOOL           IsServer   = Level->NetMode==NM_DedicatedServer || Level->NetMode==NM_ListenServer;
	UNetConnection* Connection = IsServer ? ClientConnection : GetLevel()->NetDriver->ServerConnection;
	check(Connection);

	// If saturated and function is unimportant, skip it.
	if( !(Function->FunctionFlags & FUNC_NetReliable) && !Connection->IsNetReady(0) )
		return 1;

	// Send function data to remote.
	InternalProcessRemoteFunction( this, Connection, Function, Parms, Stack, IsServer );
	return 1;

	unguardf(( TEXT("(%s)"), Function->GetFullName() ));
}

// Replicate a function call to a demo recording file
void AActor::ProcessDemoRecFunction( UFunction* Function, void* Parms, FFrame* Stack )
{
	guard(AActor::ProcessDemoRecFunction);

	// Check if the function is replicatable
	if( (Function->FunctionFlags & (FUNC_Static|FUNC_Net))!=FUNC_Net || bNetTemporary )
		return;

	UBOOL IsNetClient = (GetLevel()->GetLevelInfo()->NetMode == NM_Client);

	// Check if actor was spawned locally in a client-side demo 
	if(IsNetClient && Role == ROLE_Authority)
		return;

	// See if UnrealScript replication condition is met.
	while( Function->GetSuperFunction() )
		Function = Function->GetSuperFunction();

	UBOOL Val=0;
	if(IsNetClient)
		Exchange(RemoteRole, Role);
	bDemoRecording = 1;
	bClientDemoRecording = IsNetClient;
	FFrame( this, Function->GetOwnerClass(), Function->RepOffset, NULL ).Step( this, &Val );
	bDemoRecording = 0;
	bClientDemoRecording = 0;
	if(IsNetClient)
		Exchange(RemoteRole, Role);
	bClientDemoNetFunc = 0;
	if( !Val )
		return;

	// Get the channel.
	UNetConnection* Connection = GetLevel()->DemoRecDriver->ClientConnections(0);
	check(Connection);

	// Send function data to remote.
	BYTE* SavedCode = Stack ? Stack->Code : NULL;
	InternalProcessRemoteFunction( this, Connection, Function, Parms, Stack, 1 );
	if( Stack )
		Stack->Code = SavedCode;

	unguardf(( TEXT("(%s/%s)"), GetName(), Function->GetFullName() ));
}

/*-----------------------------------------------------------------------------
	GameInfo
-----------------------------------------------------------------------------*/

//
// Network
//
void AGameInfo::execGetNetworkNumber( FFrame& Stack, RESULT_DECL )
{
	guard(AGameInfo::execNetworkNumber);
	P_FINISH;

	*(FString*)Result = XLevel->NetDriver ? XLevel->NetDriver->LowLevelGetNetworkNumber() : FString(TEXT(""));

	unguardexec;
}

//
// Deathmessage parsing.
//
void AGameInfo::execParseKillMessage( FFrame& Stack, RESULT_DECL )
{
	guard(AGameInfo::execParseKillMessage);
	P_GET_STR(KillerName);
	P_GET_STR(VictimName);
	P_GET_STR(WeaponName);
	P_GET_STR(KillMessage);
	P_FINISH;

	FString Message, Temp;
	INT Offset;

	Temp = KillMessage;

	Offset = Temp.InStr(TEXT("%k"));
	if (Offset != -1)
	{
		Message = Temp.Left(Offset);
		Message += KillerName;
		Message += Temp.Right(Temp.Len() - Offset - 2);
	}
	Temp = Message;

	Offset = Temp.InStr(TEXT("%o"));
	if (Offset != -1)
	{
		Message = Temp.Left(Offset);
		Message += VictimName;
		Message += Temp.Right(Temp.Len() - Offset - 2);
	}
	Temp = Message;

	Offset = Temp.InStr(TEXT("%w"));
	if (Offset != -1)
	{
		Message = Temp.Left(Offset);
		Message += WeaponName;
		Message += Temp.Right(Temp.Len() - Offset - 2);
	}

	*(FString*)Result = Message;

	unguardexec;
}

/*-----------------------------------------------------------------------------
	ADecal Implementation
-----------------------------------------------------------------------------*/

// Find the coplanar surface corresponding to this intersection point.
static INT FindCoplanarSurface( UModel* Model, INT iNode, FVector IntersectionPoint, INT Depth )
{
	guard(FindCoplanarSurface);
	if( iNode == INDEX_NONE )
		return INDEX_NONE;

	FBspNode* Node = &Model->Nodes( iNode );
	if( Node->NumVertices > 0)
	{
		// check if this intersection point lies inside this node.
		FVert* Verts = &Model->Verts( Node->iVertPool );
		FVector &SurfNormal = Model->Vectors( Model->Surfs( Node->iSurf).vNormal );

		FVector* PrevVertex = &Model->Points( Verts[Node->NumVertices - 1].pVertex );
		UBOOL Success = 1;
		FLOAT PrevDot = 0;
		for( INT i=0;i<Node->NumVertices;i++ )
		{
			FVector* Vertex = &Model->Points(Verts[i].pVertex);
			FVector ClipNorm = SurfNormal ^ (*Vertex - *PrevVertex);
			FPlane ClipPlane( *Vertex, ClipNorm );

			FLOAT Dot = ClipPlane.PlaneDot( IntersectionPoint );
			
			if( (Dot < 0.f && PrevDot > 0.f) ||
				(Dot > 0.f && PrevDot < 0.f) )
			{
				Success = 0;
				break;
			}
			PrevDot = Dot;
			PrevVertex = Vertex;
		}
		if( Success )
			return Node->iSurf;
	}

	// check next co-planars to see if it contains this intersection point.
	return FindCoplanarSurface( Model, Node->iPlane, IntersectionPoint, Depth + 1 );
	unguard;
}

static void CalcClippedNodes( UModel* Model, FBspSurf& Surf, FVector* DecalVerts, TArray<INT>& NodeArray )
{
	guard(CalcClippedNodes);

	for( INT n=0;n<Surf.Nodes.Num(); n++)
	{
		FBspNode* Node = &Model->Nodes( Surf.Nodes(n) );
		
		if( Node->NumVertices > 0)
		{
			static FVector	Pts[FBspNode::MAX_FINAL_VERTICES];
			static FLOAT	Dots[FBspNode::MAX_FINAL_VERTICES];
			int NumPts;

			for( INT i=0;i<4;i++ )
				Pts[i] = DecalVerts[i];
			NumPts = 4;

			// check if this node contains any of the decal
			FVert* Verts = &Model->Verts( Node->iVertPool );
			FVector &SurfNormal = Model->Vectors( Surf.vNormal );

			FVector* PrevVertex = &Model->Points( Verts[Node->NumVertices - 1].pVertex );
			UBOOL Success = 1;
			for( INT i=0;i<Node->NumVertices;i++ )
			{
				FVector* Vertex = &Model->Points(Verts[i].pVertex);
				FVector ClipNorm = SurfNormal ^ (*Vertex - *PrevVertex);
				FPlane ClipPlane( *Vertex, ClipNorm );

				for(INT j=0;j<NumPts;j++)
					Dots[j] = ClipPlane.PlaneDot( Pts[j] );
				for( INT j=0;j<NumPts;j++ )
				{
					if(		(Dots[j] > 0 && Dots[(j+1)%NumPts] < 0) 
						||	(Dots[j] < 0 && Dots[(j+1)%NumPts] > 0))
					{
						guard(InsertClippingPoint);
						FVector NewPoint = FLinePlaneIntersection( Pts[j], Pts[(j+1)%NumPts], ClipPlane );
						if(j < NumPts-1)
						{	
							// move Dots[] and Pts[] arrays along
							appMemmove( &Dots[j+2], &Dots[j+1], sizeof(FLOAT) * (NumPts - j - 1));
							appMemmove( &Pts[j+2], &Pts[j+1], sizeof(FVector) * (NumPts - j - 1));
						}
						Pts[j+1] = NewPoint;
						Dots[j+1] = 0; 
						NumPts++;
						j++;
						check(NumPts < FBspNode::MAX_FINAL_VERTICES);
						unguard;
					}			
				}
				guard(DeleteClippedPoints);
				for( INT j=0;j<NumPts;j++ )
				{
					if( Dots[j] < 0 )
					{
						appMemmove( &Dots[j], &Dots[j+1], sizeof(FLOAT) * (NumPts - j - 1));
						appMemmove( &Pts[j], &Pts[j+1], sizeof(FVector) * (NumPts - j - 1) );
						j--;
						NumPts--;
					}
				}
				unguard;
				if( NumPts == 0 )
				{
					Success = 0;
					break;
				}
				PrevVertex = Vertex;
			}
			if( Success )
				NodeArray.AddItem( Surf.Nodes(n) );
		}
	}
	unguard;
}

void ADecal::execAttachDecal( FFrame& Stack, RESULT_DECL )
{
	guard(ADecal::execAttachDecal);
	P_GET_FLOAT(TraceDistance);
	P_GET_VECTOR_OPTX(DecalDir,FVector(0,0,0));
	P_FINISH;

	*(UTexture**)Result = NULL;
	if( !GetLevel()->Engine->Client || !GetLevel()->Engine->Client->Decals )
		return;

#ifndef NODECALS
//	Enable shadows in fog, because we don't have much heavy fog.
//	Fix later if we want the shadows to fog also -- jsp.
//	if( Region.Zone->bFogZone )
//		return;
	if(!Texture)
	{
		debugf(TEXT("AttachDecal: No Texture"));
		return;
	}
	MultiDecalLevel = Min<INT>(MultiDecalLevel, 4);

	UModel *Model = Level->XLevel->Model;
	FCheckResult Hit(1.0f);
	FVector EndVect = -Rotation.Vector(); // assume rotation oriented in direction of hitnormal
	EndVect *= TraceDistance;

	INT RandDir = 0;
	if ( DecalDir.IsZero() )
	{
		DecalDir = VRand();
		RandDir = 1;
	}

	if( Model->LineCheck( Hit, NULL, Location + EndVect, Location, FVector(0, 0, 0), TRACE_VisBlocking ) != 0 ||
	    Hit.Item == INDEX_NONE )
	{
		return;
	}
	else
	{
		FBspSurf &Surf = Model->Surfs( Model->Nodes(Hit.Item).iSurf );
		FVector &SurfNormal = Model->Vectors(Surf.vNormal);
		FVector &SurfBase = Model->Points(Surf.pBase);
		FVector Intersection = FLinePlaneIntersection( Location,  Location + EndVect, SurfBase, SurfNormal );
		INT SurfIndex = FindCoplanarSurface( Model, Hit.Item, Intersection, 0 );
	
		if( SurfIndex == INDEX_NONE )
			return;
	
		// setup vertices for main decal surface.
		{
		FBspSurf &Surf = Model->Surfs(SurfIndex);
		FVector &SurfNormal = Model->Vectors(Surf.vNormal);
		FVector &SurfBase = Model->Points(Surf.pBase);
		FVector DecalCenter = FLinePlaneIntersection( Location, Location + EndVect, SurfBase, SurfNormal );

		FLOAT d = Rotation.Vector() | SurfNormal;
		if( !d )
		{
			//debugf(TEXT("AttachDecal: decal ray is parallel to surface"));
			return;
		}
		if(Abs(((SurfBase - DecalCenter) | SurfNormal)) > 0.001f )
		{
			//debugf(TEXT("AttachDecal: Couldn't place decal: dot product is %f"), ((SurfBase - DecalCenter) | SurfNormal));
			return;
		}

		if( Surf.PolyFlags & (PF_AutoUPan|PF_AutoVPan) )
			return;

		// attach decal to new surface
		FDecal* MainDecal = NULL;
		for( INT j=0;j<Surf.Decals.Num();j++)
			if(Surf.Decals(j).Actor->Texture == Texture)
			{
				Surf.Decals.InsertZeroed(j);
				MainDecal = &Surf.Decals(j);					
				break;
			}
		if(!MainDecal) 
			MainDecal = &Surf.Decals(Surf.Decals.AddZeroed());		
		MainDecal->Actor = this;
		SurfList.AddItem(SurfIndex);

		{
		FLOAT diag = appSqrt( DrawScale * DrawScale * Texture->USize * Texture->USize / 2.f );
		// calculate decal co-ordinates - ASSUME DECALS ARE SQUARE

		if ( !RandDir )
		{
			// Project DecalDir onto the surface
			FVector MainAxis = DecalDir - (DecalDir | SurfNormal) * SurfNormal;

			if ( MainAxis.IsNearlyZero() )
			{
				MainAxis = DecalDir = ( SurfBase - DecalCenter );
				RandDir = 1;
			}
			else
			{
				// then we cross with the normal to get the other axis.
				FVector OtherAxis = MainAxis ^ SurfNormal;
				MainAxis.Normalize();
				OtherAxis.Normalize();

				// calculate the vector from the center to the diagonal.
				MainDecal->Vertices[0] = MainAxis + OtherAxis;
				MainDecal->Vertices[1] = MainAxis - OtherAxis;
			}
		}
		if ( RandDir )
		{
			// calculate the vector from the center to the diagonal.
			MainDecal->Vertices[0] = DecalDir - (DecalDir | SurfNormal) * SurfNormal;
			MainDecal->Vertices[1] = MainDecal->Vertices[0] ^ SurfNormal;
		}

		MainDecal->Vertices[0].Normalize();
		MainDecal->Vertices[1].Normalize();
		MainDecal->Vertices[0] *= diag;
		MainDecal->Vertices[1] *= diag;
		MainDecal->Vertices[2] = -MainDecal->Vertices[0];
		MainDecal->Vertices[3] = -MainDecal->Vertices[1];
		MainDecal->Vertices[0] += DecalCenter;
		MainDecal->Vertices[1] += DecalCenter;
		MainDecal->Vertices[2] += DecalCenter;
		MainDecal->Vertices[3] += DecalCenter;
		CalcClippedNodes( Model, Surf, MainDecal->Vertices, MainDecal->Nodes );
		}

		{
		FLOAT NormSize = SurfNormal.Size();
		FVector TraceVect = -50*(SurfNormal / NormSize);
		FVector XVect = MainDecal->Vertices[1] - MainDecal->Vertices[0];
		FVector YVect = MainDecal->Vertices[3] - MainDecal->Vertices[0];

		for( INT X=0; X < MultiDecalLevel; X++ )
		{
			for( INT Y=0; Y < MultiDecalLevel; Y++ )
			{
				FVector TracePoint = MainDecal->Vertices[0] + (((FLOAT)(X+1.))/MultiDecalLevel)*XVect + (((FLOAT)(Y+1.))/MultiDecalLevel)*YVect;
				if( Model->LineCheck( Hit, NULL, TracePoint + TraceVect, TracePoint - TraceVect, FVector(0, 0, 0), TRACE_VisBlocking ) == 0 && Hit.Item != INDEX_NONE )
				{
					FBspSurf &SecSurf = Model->Surfs( Model->Nodes(Hit.Item).iSurf );
					FVector &SecNormal = Model->Vectors(SecSurf.vNormal);
					FVector &SecBase = Model->Points(SecSurf.pBase);
					FVector SecInt = FLinePlaneIntersection( TracePoint - TraceVect,  TracePoint + TraceVect, SecBase, SecNormal );
					SurfIndex = FindCoplanarSurface( Model, Hit.Item, SecInt, 0 );
				}
				else
					continue;

				if( SurfIndex == INDEX_NONE )
					continue;

				FBspSurf &SecSurf = Model->Surfs(SurfIndex);
				FVector &SecNormal = Model->Vectors(SecSurf.vNormal);
				FVector &SecBase = Model->Points(SecSurf.pBase);

				INT Found;
				if(SurfList.FindItem(SurfIndex, Found))
					continue;

				if( SecSurf.PolyFlags & (PF_AutoUPan|PF_AutoVPan) )
					continue;

				FLOAT costheta = (SurfNormal | SecNormal) / (SurfNormal.Size() * SecNormal.Size());
				if( Abs(costheta) <= 0.7 ) 
					continue;	// angle is too close to 90 degrees	

				// attach decal to secondary surface
				FDecal* SecDecal = NULL;
				for( INT j=0;j<SecSurf.Decals.Num();j++)
					if(SecSurf.Decals(j).Actor->Texture == Texture)
					{
						SecSurf.Decals.InsertZeroed(j);
						SecDecal = &SecSurf.Decals(j);					
						break;
					}
				if(!SecDecal) 
					SecDecal = &SecSurf.Decals(SecSurf.Decals.AddZeroed());
				SecDecal->Actor = this;
				SurfList.AddItem(SurfIndex);

				for( INT j=0;j<4;j++)
				{
					// Locate texture-wrapped point on secondary surface, for each vertex
					FVector A = FLinePlaneIntersection( MainDecal->Vertices[j]-SurfNormal, MainDecal->Vertices[j], SecBase, SecNormal );
					FVector B = FLinePlaneIntersection( MainDecal->Vertices[j]-SecNormal, MainDecal->Vertices[j], SecBase, SecNormal );
					FLOAT X = (MainDecal->Vertices[j] - B).Size() / costheta;
					FLOAT H = (MainDecal->Vertices[j] - A).Size() / costheta;
					FVector AB = B - A;
					AB.Normalize();
					SecDecal->Vertices[j] = B - (H-X)*AB;
				}
				CalcClippedNodes( Model, SecSurf, SecDecal->Vertices, SecDecal->Nodes );
				SecDecal->Vertices[0] -= SecBase;
				SecDecal->Vertices[1] -= SecBase;
				SecDecal->Vertices[2] -= SecBase;
				SecDecal->Vertices[3] -= SecBase;
			}
		}
		}

		MainDecal->Vertices[0] -= SurfBase;
		MainDecal->Vertices[1] -= SurfBase;
		MainDecal->Vertices[2] -= SurfBase;
		MainDecal->Vertices[3] -= SurfBase;
		*(UTexture**)Result = Surf.Texture;
		}
	}
#endif
	unguard;
}

void ADecal::execDetachDecal( FFrame& Stack, RESULT_DECL )
{
	guard(ADecal::execDetachDecal);
	P_FINISH;

#ifndef NODECALS
	while( SurfList.Num() > 0 )
	{
		// detach decal from old surface
		FBspSurf& Surf = Level->XLevel->Model->Surfs(SurfList(SurfList.Num()-1));
		UBOOL RemovedDecal = 0;
		for( INT i=0; i<Surf.Decals.Num(); i++ )
			if( Surf.Decals(i).Actor == this )
			{
				Surf.Decals.Remove(i);
				RemovedDecal = 1;
				break;
			}

		//!! check(RemovedDecal);  // caused a crash with shadows during GC...
		SurfList.Remove(SurfList.Num()-1);
	}
#endif
	unguard;
}

// Color functions
#define P_GET_COLOR(var)            P_GET_STRUCT(FColor,var)

void AActor::execMultiply_ColorFloat( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execMultiply_ColorFloat);

	P_GET_COLOR(A);
	P_GET_FLOAT(B);
	P_FINISH;

	A.R = (BYTE) (A.R * B);
	A.G = (BYTE) (A.G * B);
	A.B = (BYTE) (A.B * B);
	*(FColor*)Result = A;

	unguardexecSlow;
}	

void AActor::execMultiply_FloatColor( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execMultiply_FloatColor);

	P_GET_FLOAT (A);
	P_GET_COLOR(B);
	P_FINISH;

	B.R = (BYTE) (B.R * A);
	B.G = (BYTE) (B.G * A);
	B.B = (BYTE) (B.B * A);
	*(FColor*)Result = B;

	unguardexecSlow;
}	

void AActor::execAdd_ColorColor( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execAdd_ColorColor);

	P_GET_COLOR(A);
	P_GET_COLOR(B);
	P_FINISH;

	A.R = A.R + B.R;
	A.G = A.G + B.G;
	A.B = A.B + B.B;
	*(FColor*)Result = A;

	unguardexecSlow;
}

void AActor::execSubtract_ColorColor( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execSubtract_ColorColor);

	P_GET_COLOR(A);
	P_GET_COLOR(B);
	P_FINISH;

	A.R = A.R - B.R;
	A.G = A.G - B.G;
	A.B = A.B - B.B;
	*(FColor*)Result = A;

	unguardexecSlow;
}

void AActor::execCreateTextureFromScreenShot( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execCreateTextureFromScreenShot);
	P_GET_OBJECT(UViewport, viewport);
	P_FINISH;

	*(UTexture**)Result = NULL;

	viewport->Exec(TEXT("snap 8"));

	UTexture* newTexture = new UTexture ();
	newTexture->LoadFromSnap (viewport, 128, 64);

	*(UTexture**)Result = newTexture;

	unguardSlow;
}


void AActor::execCreateTextureFromBMP( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execCreateTextureFromScreenShot);
	P_GET_STR(Name);
	P_GET_STR(Text);
	P_FINISH;

	*(UTexture**)Result = NULL;
/*
	UTexture* newTexture = new UTexture ();
	if (newTexture->LoadFromBMP (Text))
		*(UTexture**)Result = newTexture;
	else
		newTexture->Destroy();
*/
	FName PkgName = TEXT("SavePics");
	UPackage* Pkg = CreatePackage(NULL,*PkgName);

	UTexture* Texture = ImportObject<UTexture>( Pkg, *Name, RF_Public|RF_Standalone, *Text );
	if (Texture)
		*(UTexture**)Result = Texture;

	unguardSlow;
}

void AActor::execSaveObjectAsFile( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execCreateTextureFromScreenShot);
	P_GET_STR(Dir);
	P_GET_OBJECT(UObject, object);
	P_FINISH;

	FArchive* Ar = GFileManager->CreateFileWriter( *Dir );
	if( Ar )
	{
		// save object
		object->Serialize (*Ar);

		delete Ar; // success

		*(bool*)Result = true;
	}
	else
		*(bool*)Result = false;

	unguardSlow;
}

// Sto: Couldn't get the following function to work
// Would be fairly useful if it did, so left unfinished code for now
void AActor::execLoadObjectAsFile( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execCreateTextureFromScreenShot);
	P_GET_STR(Dir);
	P_GET_OBJECT(UObject, object);
	P_FINISH;

	FArchive * Ar = GFileManager->CreateFileReader( *Dir );
	if( Ar )
	{
		// save object
		object->Serialize (*Ar);

		delete Ar; // success

		*(bool*)Result = true;
	}
	else
		*(bool*)Result = false;

	unguardSlow;
}

static void SerializeInfo(FArchive * Ar, UGameSaveInfo * info)
{
	(*Ar) << info->numBeans << info->numStars << info->numPoints;

	// AWRIGHT_111001_001
	(*Ar) << info->savePointID;

	(*Ar) << info->currentLevelString;
}

void AActor::execSaveGameSaveInfo( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execCreateTextureFromScreenShot);
	P_GET_STR(Dir);
	P_GET_OBJECT(UGameSaveInfo, info);
	P_FINISH;

 	FArchive* Ar = GFileManager->CreateFileWriter( *(GSys->SaveSlotPath*Dir) );
	if( Ar )
	{
		SerializeInfo(Ar, info);
		delete Ar; // success

		*(bool*)Result = true;
	}
	else
		*(bool*)Result = false;

	unguardSlow;
}

void AActor::execLoadGameSaveInfo( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execCreateTextureFromScreenShot);
	P_GET_STR(Dir);
	P_GET_OBJECT(UGameSaveInfo, info);
	P_FINISH;

	if (!info)
	{
		*(bool*)Result = false;
		return;
	}


	FArchive * Ar = GFileManager->CreateFileReader(*(GSys->SaveSlotPath*Dir));
	if( Ar )
	{
		SerializeInfo(Ar, info);
		delete Ar; // success

		*(bool*)Result = true;
	}
	else
		*(bool*)Result = false;

	unguardSlow;
}

#if defined(_WIN32) || defined(WIN32)
CORE_API bool IsOSVer2kOrXP ();
#else
CORE_API bool IsOSVer2kOrXP()
{
	return false;
}
#endif


void AActor::execIsOSVer2kOrXP( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execIsOSVer2kOrXP);
	P_FINISH;

	*(bool*)Result = IsOSVer2kOrXP ();
	
	unguardSlow;
}

void AActor::execIsSoftwareRendering( FFrame& Stack, RESULT_DECL )
{
	guardSlow(AActor::execIsSoftwareRendering);
	P_FINISH;

	FString	str;
	GConfig->GetString(TEXT("Engine.Engine"), TEXT("GameRenderDevice"), str);

	const UBOOL IsSoftwareRendering = appStricmp(*str, TEXT("SoftDrv.SoftwareRenderDevice")) == 0;
	*(bool*)Result = IsSoftwareRendering;
	appRecordRandTracePreBeginSoftwareRendering( this, IsSoftwareRendering );
	HP2CreatureGeneratorTraceSoftwareRendering( this, IsSoftwareRendering );
	unguardSlow;
}

/*-----------------------------------------------------------------------------
	The End.
-----------------------------------------------------------------------------*/
