//===============================================================================
//  [WizardCardIcon] 
//===============================================================================

class WizardCardIcon extends HProp abstract;



var rotator newrot;

var vector	StartPoint;
var float	fHeight;
var	float	fTimeToTargetPoint;
var float	fTimeToWait;
var	float	fStartTimeToTargetPoint;

var string WizardName;
var int	   ID;
var bool   bVendorsCanSell;
var string strVendorOwnedAfterGState;

var WizCardSpin		SpinFX;

var vector	PreviousLocation;
var bool	bBouncingState;

var texture textureBig;
var string  strDescriptionId;

	//CMP: Support for layeres in the gold wizard cards.
var bool bIsLayered;
var bool bLastLayerIsFire;
var texture textureLayers[3];

function PreBeginPlay()
{
	Super.PreBeginPlay ();
}

function Spawned()
{
	local ParticleFX	explosion;

	SetPhysics(PHYS_Falling);
	bBouncingState = true;
	//PlaySound(sound'HpSounds.magic_sfx.pickup_wizardcard', SLOT_TALK);
	//PlaySound(sound'HpSounds.magic_sfx.wizardcard_rotate');
	explosion = spawn(class'WizCard_explo', [spawnlocation] location);
	explosion = spawn(class'Spawn_Flash_1', [spawnlocation] location);

	gotostate('bouncing');
}

function PostBeginPlay()
{
	Super.PostBeginPlay ();

}

function Touch (actor other)
{
	local Harry                 harry;
	local class<StatusItem>     classStatusItem;
	local StatusItemWizardCards siCard;
	local int                   nNewCardCount;
    local int                   nOldCardCount;

	harry = Harry(other);
	if (harry == none)
		return;

	// Get classStatusItem that card corresponds with and update array 
	// of card data in Harry with this card type.
	if (isa('BronzeCards'))
		classStatusItem = class'StatusItemBronzeCards';
	else if (isa('SilverCards'))
		classStatusItem = class'StatusItemSilverCards';
	else if (isa('GoldCards'))
		classStatusItem = class'StatusItemGoldCards';
	else
		harry.ClientMessage("Error WizardCard class " $class $" not recognized");

	// If just finished a set of bronze cards (1 set == 10 cards), celebrate!
	siCard = StatusItemWizardCards(harry.managerStatus.GetStatusItem(class'StatusGroupWizardCards', classStatusItem));
    nOldCardCount = siCard.nCount;
	siCard.SetCardOwner(ID, siCard.ECardOwner.CardOwner_Harry);
	nNewCardCount = siCard.nCount;
    if (isa('BronzeCards') && (nOldCardCount != nNewCardCount))
    {
	    if (((nNewCardCount % 10) == 0))
		    harry.DoCelebrateBronzeCardSet();
    }

	// Super.Touch handles flying things to hud.
	Super.Touch(other);
}

state bouncing
{
	function HitWall( vector HitNormal, actor Wall )
	{
		log("WizardIcon: Hit wall");
		Velocity *= 0.5;
		Velocity = MirrorVectorByNormal( Velocity, HitNormal );
		
		if (vsize(previousLocation - location) < 0.1)
		{
			SetPhysics(PHYS_None);
			gotostate('wait');
		}
		previousLocation = location;
	}

	function tick (float delta)
	{

		newrot=rotation;
		newrot.yaw=newrot.yaw+(50000*delta);
	
		setrotation(newrot);
	}

// This year, we don't want to wait until the card finishes bouncing before
// Harry can pick it up.  But, that also means that if the card hits Harry
// while flying out of the chest, it will go to the hud right away.  If 
// we want to change things back, just uncomment this function.
/*
	function touch(actor other)
	{
		if (other.isa('harry'))
		{
			Velocity *= 0.5;
			Velocity = -Velocity;
		}
	}
*/
}

auto state wait
{

	function HitWall( vector HitNormal, actor Wall )
	{
/*		Velocity *= 0.5;
		Velocity = MirrorVectorByNormal( Velocity, HitNormal );*/
	}
		

	function tick (float delta)
	{
		if (bBouncingState)
		{
			bBouncingState = false;
			gotostate('bouncing');
		}

		newrot=rotation;
		newrot.yaw=newrot.yaw+(50000*delta);
		
		setrotation(newrot);
	}

	begin:
	sleep(1);
	goto 'begin';
}

defaultproperties
{
    fHeight=50
    fTimeToTargetPoint=3.2
    fTimeToWait=3
    WizardName="Unknown"
	ID=0
	bVendorsCanSell=false
    bStatic=False
    DrawType=DT_Mesh
    Mesh=SkeletalMesh'HProps.skWizardCardIconMesh'
    DrawScale=2
    AmbientGlow=250
	bcollideactors=true
	bcollideworld=true

	bBlockActors=false
	bBlockPlayers=false

    bBounce=true

	// Although wizard cards fly up to the hud, their count will not be
	// tracked in the standard manner.  We also need to keep track
	// of which cards have been picked up- will directly deal with the
	// status manager classes to do this.
	nPickupIncrement=0

	// Need this set to false or the card won't rotate while physics is Falling.
	bRotateToDesired=false
}
