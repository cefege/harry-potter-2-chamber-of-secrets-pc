//=============================================================================
// GridMover.
//=============================================================================
class GridMover extends Mover;

// Allows this mover to go anywhere on a grid, depending on the direction it is hit.

var() float MoveIncrement;
var() float OmniGridSize;
var() bool  bOmniGridMover;

var   bool  bDoingInterpolation;
var   bool  bContinueMove;

var float	NewMoveIncrement;
var vector  NewStartPosition;
var vector  SavedNormal;

function bool VectorIsZero(vector v)
{
	return v.X==0.f && v.Y==0.f && v.Z==0.f;
}

function Tick(float dtime)
{
	super.Tick(dtime);

	//Check for Mover bug, if in the middle of moving, but for some reason bInterpolating got
	// turned off, stop moving.
	if( bDoingInterpolation && !bInterpolating )
	{
		// if omni grid mover hits something, continue to move
		if(bOmniGridMover && !VectorIsZero(HitNormal))
		{
			SavedNormal		= HitNormal;
			HitNormal		= vec(0, 0, 0);
			bContinueMove	= true;
		}

		GotoState('BumpMove', 'DoneMoving');
	}
}

function INT myRound(float f)
{
	local INT result;
	result = f;
	return result;
}

// Move when bumped.
state() BumpMove
{
	function Bump( actor Other )
	{
		local vector offset;
		local vector delta;
		local float length;

		//Let super handle the special trigger stuff.
		super.Bump( Other );

		//Log("**************** a:"$self$" GridMover 8 Other:"$Other$" Other.I:"$Other.Instigator);
		if( !IsRelevant(Other) )
			return;

		//Log("**************** a:"$self$" GridMover 9");
		SavedTrigger = Other;
		Instigator = Pawn( Other );

		// Dynamically set interpolation point.
		KeyPos[1] = Location-BasePos;

		// remember it to use, if hit something
		NewStartPosition = Location;
		NewMoveIncrement = MoveIncrement;

		offset = Other.Location - Location;

		length = offset.X * offset.X + offset.Y * offset.Y;
		length = Sqrt(length);

		if(length > 1)
		{
			if(!bOmniGridMover)	// this is a "regular" grid mover (in X or Y direction)
			{
				if( Abs(offset.X) > Abs(offset.Y) )
				{
					if( offset.X > 0 )
						KeyPos[1].X -= MoveIncrement;
					else
						KeyPos[1].X += MoveIncrement;
				}
				else
				{
					if( offset.Y > 0 )
						KeyPos[1].Y -= MoveIncrement;
					else
						KeyPos[1].Y += MoveIncrement;
				}
			}
			else				// this is an omni grid mover (in arbitrary direction)
			{
				delta.X = offset.X * MoveIncrement / length;
				delta.Y = offset.Y * MoveIncrement / length;

				KeyPos[1].X -= delta.X;
				KeyPos[1].Y -= delta.Y;

				KeyPos[1].X = OmniGridSize * myRound(KeyPos[1].X / OmniGridSize);
				KeyPos[1].Y = OmniGridSize * myRound(KeyPos[1].Y / OmniGridSize);
			}
		}

		GotoState( 'BumpMove', 'Move' );
	}

  Move:
	Disable( 'Bump' );

	bDoingInterpolation = true;

	if ( DelayTime > 0 )
	{
		bDelaying = true;
		Sleep(DelayTime);
	}

	DoOpen();

	FinishInterpolation();

  DoneMoving:
  	bDoingInterpolation = false;

	// to fix a bug, when grid mover stop before interpolation is done,
	// and continue to play ambient sound. Stop this sound here.
	StopSound(MoveAmbientSound, SLOT_Misc);

	FinishedOpening();

	KeyNum = 0; PrevKeyNum = 0;
	Sleep( StayOpenTime );

	if( bTriggerOnceOnly )
		GotoState('');

	Enable( 'Bump' );

	if(bContinueMove)
	{
		bContinueMove = false;
		GotoState('ContinueMove');
	}
}

// looking for a new unit vector with the same angle from the normal
function vector GetNewLocation(vector oldLoc, vector hitLoc, vector n)
{
	local vector v, u, w;
	local float t, q2, r, lv, lv2, vn;
	local float c1, c2;
	local float a, b, c, d, d2, x1, x2;

	v	= oldLoc - hitLoc;
	v.Z = 0;

	// if it is against the wall already, put it back in direction of normal.
	if( VectorIsZero(v) )
	{
		w.X = NewMoveIncrement * n.X;
		w.Y = NewMoveIncrement * n.Y;
		w.Z = 0;
	}
	else
	{
		lv2	= v.X * v.X + v.Y * v.Y;
		lv	= Sqrt(lv2);
		t = Sqrt((NewMoveIncrement * NewMoveIncrement) / lv2 - 1);

		NewMoveIncrement = NewMoveIncrement - lv;

		// make it length equal to 1.
		v.X = v.X / lv;
		v.Y = v.Y / lv;

		q2 = 1;
		vn = n.X * v.X + n.Y * v.Y;
		r  = vn;

		w.Z = 0;

		if(n.Y != 0)
		{
			c1 = r / n.Y;
			c2 = n.X / n.Y;
			a = 1 + c2 * c2;
			b = c1 * c2;
			c = c1 * c1 - q2;
			d2 = b * b - a * c;
			d = Sqrt(d2);

			x1 = (b + d) / a;
			x2 = (b - d) / a;

			if(abs(v.x - x1) < 0.01)
				w.x = x2;
			else
				w.x = x1;

			w.y = (vn - w.x * n.X) / n.Y;
		}
		else
		{
			w.X = vn / n.X;
			w.Y = Sqrt(1 - w.X * w.X);

			if(v.Y > 0)
				w.Y = -w.Y;
		}
	}

	w.X = w.X * lv * t;
	w.Y = w.Y * lv * t;

	return w;
}

function NextMove()
{
	local vector w;

	w = GetNewLocation(NewStartPosition, Location, SavedNormal);

	log("w             X = " $w.X);
	log("w             Y = " $w.Y);

	// Dynamically set interpolation point.
	KeyPos[1] = Location - BasePos;
	NewStartPosition = Location;

	KeyPos[1].X += w.X;
	KeyPos[1].Y += w.Y;

	Disable( 'Bump' );

	bDoingInterpolation = true;
	DoOpen();
}

// Continue to move, if hit something.
state ContinueMove
{

Begin:
	NextMove();
	FinishInterpolation();
}

defaultproperties
{
	StayOpenTime=0
	eVulnerableToSpell=SPELL_Flipendo
	bProjTarget=true
	
	MoveIncrement=64.000000
	OmniGridSize=2.000000
	bOmniGridMover=false

	bContinueMove=false

	InitialState=BumpMove
	CollisionRadius=55.000000
	CollisionHeight=48.000000
	bCollideWorld=True
}
