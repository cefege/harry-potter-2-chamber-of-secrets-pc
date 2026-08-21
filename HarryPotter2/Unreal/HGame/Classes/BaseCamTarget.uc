class BaseCamTarget expands HiddenHPawn;

var actor	aAttachedTo;	// actor that the target is attached to, may be none
var vector	vOffset;		// offset from the actor we are attached to
var bool	bRelative;		// weather or not the target's offset is reletive or not

var BaseCam Cam;            // BaseCamTarget doesn't set Owner to BaseCam anymore, the BaseCam's Owner is BaseCamTarget.
                            //  so, get a ref to BaseCam so we do Cut cue's and what not...

function bool	IsAttached()						{ return (aAttachedTo != None); }
function		SetOffset		( vector v	)		{ vOffset     = v;	UpdateOrientation(); }
function		SetXOffset		( float x	)		{ vOffset.x	  = x;	UpdateOrientation(); }
function		SetYOffset		( float y	)		{ vOffset.y	  = y;	UpdateOrientation(); }
function		SetZOffset		( float z	)		{ vOffset.z	  = z;	UpdateOrientation(); }

function SetAttachedTo	( actor a	)
{
	aAttachedTo = a;
	UpdateOrientation();

	//Make sure who ever this BaseCamTarget is attached to gets ticked first
	TickParent = aAttachedTo;
}

function bool SetAttachedToByName( name nName ) 
{
	local Actor A;
	// Look for the new target
	if( nName != '' )
	ForEach AllActors(class'Actor', A)
	{
		if( A.Name == nName || A.CutName ~= string(nName) )
		{
			aAttachedTo = A;
			if( bRelative )
				SetLocation( aAttachedTo.location + (vOffset >> aAttachedTo.rotation) );
			else
				SetLocation( aAttachedTo.location + vOffset );

			//Make sure who ever this BaseCamTarget is attached to gets ticked first
			TickParent = aAttachedTo;

			playerHarry.ClientMessage("CamTarget is AttachedTo: " $aAttachedTo );

			return true;
		}
	}
	// Let the user know what names you can use
	playerHarry.ClientMessage("Could not find targetActor with the name -> " $nName );
	playerHarry.ClientMessage("Valid names you can use are as follows:");
	ForEach AllActors(class'Actor', A)
		playerHarry.ClientMessage(A);
	return false;
}

function bool SetAttachedToByCutName( string sCutName )
{
	local Actor A;
	
	if( sCutName ~= "none" )
	{
		aAttachedTo = none;
		return true;
	}
	
	// Look for the new target
	ForEach AllActors(class'Actor', A)
	{
		if( A.CutName ~= sCutName || string(A.Name) ~= sCutName )
		{
			aAttachedTo = A;
			if( bRelative )
				SetLocation( aAttachedTo.location + (vOffset >> aAttachedTo.rotation) );
			else
				SetLocation( aAttachedTo.location + vOffset );

			//Make sure who ever this BaseCamTarget is attached to gets ticked first
			TickParent = aAttachedTo;
			
			return true;
		}
	}

	Cam.CutErrorString = "baseCam target could not find the actor with the CutName: " $sCutName;
	return false;
}

function SetNewRotation( rotator rot )
{
	DesiredRotation		  = rot;
	DesiredRotation.Yaw	  = DesiredRotation.Yaw		& 0xFFFF;
	DesiredRotation.Pitch = DesiredRotation.Pitch	& 0xFFFF;
	DesiredRotation.Roll  = DesiredRotation.Roll	& 0xFFFF;
	SetRotation( DesiredRotation );
}

event Tick( float fTimeDelta )
{
	Super.Tick( fTimeDelta );
	UpdateOrientation();
}

function UpdateOrientation()
{
	local vector delta;	// for steps up/down

	if( aAttachedTo != None )
	{
		delta = vec(0, 0, aAttachedTo.SavedPrePivotZ);

		// --- Update Rotation
		SetNewRotation( aAttachedTo.rotation );

		// --- Update Position
		if( bRelative )
			SetLocation( aAttachedTo.location + delta + (vOffset >> rotation) );
		else
			SetLocation( aAttachedTo.location + delta + vOffset);
	}
}

defaultproperties
{
	bIgnoreZonePainDamage=true
}