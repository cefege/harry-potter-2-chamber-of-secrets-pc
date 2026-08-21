//=============================================================================
// SpellLessonInterpolationPoint  -- Special interpolation point used
// for spell lesson "game".
//=============================================================================

class SpellLessonInterpolationPoint extends InterpolationPoint;

const nIDX_IDLE = 0;
const nIDX_HIT  = 1;
const nIDX_MISS = 2;

enum EDirectionArrow
{
    Arrow_Up,
    Arrow_Down,
    Arrow_Left,
    Arrow_Right,
    Arrow_None
};

var() EDirectionArrow DirectionArrow[3];
var   bool            bDeactivated;
var   sound           soundHitArrowUp;
var   sound           soundHitArrowDown;
var   sound           soundHitArrowLeft;
var   sound           soundHitArrowRight;
var   sound           soundMissed;
var   texture         UpTextures[3];
var   texture         DownTextures[3];
var   texture         LeftTextures[3];
var   texture         RightTextures[3];

function PostBeginPlay()
{
	UpTextures[nIDX_IDLE]    = texture(DynamicLoadObject("SpellShapes.Shapes.spar_up_idle", class'Texture'));
    UpTextures[nIDX_HIT]     = texture(DynamicLoadObject("SpellShapes.Shapes.spar_up_hit", class'Texture'));
    UpTextures[nIDX_MISS]    = texture(DynamicLoadObject("SpellShapes.Shapes.spar_up_miss", class'Texture'));

	DownTextures[nIDX_IDLE]  = texture(DynamicLoadObject("SpellShapes.Shapes.spar_dn_idle", class'Texture'));
    DownTextures[nIDX_HIT]   = texture(DynamicLoadObject("SpellShapes.Shapes.spar_dn_hit", class'Texture'));
    DownTextures[nIDX_MISS]  = texture(DynamicLoadObject("SpellShapes.Shapes.spar_dn_miss", class'Texture'));

    LeftTextures[nIDX_IDLE]  = texture(DynamicLoadObject("SpellShapes.Shapes.spar_lt_idle", class'Texture'));
    LeftTextures[nIDX_HIT]   = texture(DynamicLoadObject("SpellShapes.Shapes.spar_lt_hit", class'Texture'));
    LeftTextures[nIDX_MISS]  = texture(DynamicLoadObject("SpellShapes.Shapes.spar_lt_miss", class'Texture'));

	RightTextures[nIDX_IDLE] = texture(DynamicLoadObject("SpellShapes.Shapes.spar_rt_idle", class'Texture'));
    RightTextures[nIDX_HIT]  = texture(DynamicLoadObject("SpellShapes.Shapes.spar_rt_hit", class'Texture'));
    RightTextures[nIDX_MISS] = texture(DynamicLoadObject("SpellShapes.Shapes.spar_rt_miss", class'Texture'));

    SetArrowTexture(nIDX_IDLE, 0);
}

function OnPlayerHit(int nLevel)
{
	bDeactivated = true;
    SetArrowTexture(nIDX_HIT, nLevel);
    fancyspawn(class'SpellLessonHit');
    switch (DirectionArrow[nLevel])    
    {
    case (Arrow_Up)    : PlaySound(soundHitArrowUp, , , , 10000, , true);    break;
    case (Arrow_Down)  : PlaySound(soundHitArrowDown, , , , 10000, , true);  break;
    case (Arrow_Left)  : PlaySound(soundHitArrowLeft, , , , 10000, , true);  break;
    case (Arrow_Right) : PlaySound(soundHitArrowRight, , , , 10000, , true); break;
    default :
        break;
    }
}

function OnPlayerMissed(int nLevel)
{
	bDeactivated = true;    
    SetArrowTexture(nIDX_MISS, nLevel);
    fancyspawn(class'SpellLessonMiss');
	PlaySound(soundMissed, SLOT_None);
}

function Reset(int nLevel)
{
    local bool bDisplay;

    SetArrowTexture(nIDX_IDLE, nLevel);
	bDeactivated = false;
    
	if (IsInLevel(nLevel))
		bHidden = false;
    else
    {
        bHidden = true;
        bDeactivated = true;
    }
}

// Return true if IP is in the level passed in
function bool IsInLevel(int nLevel)
{
    local bool bRet;

    if (VerifyLevel(nLevel))
        return (DirectionArrow[nLevel] != Arrow_None);
    else
        return (false);
}

function bool VerifyLevel(int nLevel)
{
    if ((nLevel >= 0) && (nLevel < ArrayCount(DirectionArrow)))
        return (true);
    else
    {
        log("ERROR:  Invalid nLevel passed to IsInLevel " $nLevel);
        return (false);
    }
}

function bool IsActive(int nLevel)
{
    if (VerifyLevel(nLevel))
        return ((DirectionArrow[nLevel] != Arrow_None) && !bDeactivated);
    else
        return (false);
}

function bool IsDirectionArrowUp(int nLevel)
{
    if (VerifyLevel(nLevel))
        return (DirectionArrow[nLevel] == Arrow_Up);
    else
        return (false);
}

function bool IsDirectionArrowDown(int nLevel)
{
    if (VerifyLevel(nLevel))
        return (DirectionArrow[nLevel] == Arrow_Down);
    else
        return (false);
}

function bool IsDirectionArrowLeft(int nLevel)
{
    if (VerifyLevel(nLevel))
        return (DirectionArrow[nLevel] == Arrow_Left);
    else
        return (false);
}

function bool IsDirectionArrowRight(int nLevel)
{
    if (VerifyLevel(nLevel))
        return (DirectionArrow[nLevel] == Arrow_Right);
    else
        return (false);
}

function SetArrowTexture(int nIdx, int nLevel)
{
    if (!VerifyLevel(nLevel))
        return;

    if ((nIdx < 0) || (nIdx >= ArrayCount(UpTextures)))
    {
        log("ERROR: Invalid SpellLessonInterpolationPoint texture index " $nIdx);
        return;
    }
    
    switch (DirectionArrow[nLevel])    
    {
    case (Arrow_Up)    : MultiSkins[0] = UpTextures[nIdx];    break;
    case (Arrow_Down)  : MultiSkins[0] = DownTextures[nIdx];      break;
    case (Arrow_Left)  : MultiSkins[0] = LeftTextures[nIdx];    break;
    case (Arrow_Right) : MultiSkins[0] = RightTextures[nIdx];   break;
    default :
        break;
    }
}

function texture GetArrowTexture(int nIdx, int nLevel)
{
    if (!VerifyLevel(nLevel))
        return (None);

    if ((nIdx < 0) || (nIdx >= ArrayCount(UpTextures)))
    {
        log("ERROR: Invalid SpellLessonInterpolationPoint texture index " $nIdx);
        return (None);
    }
    
    switch (DirectionArrow[nLevel])    
    {
    case (Arrow_Up)    : return (LeftTextures[nIdx]);
    case (Arrow_Down)  : return (UpTextures[nIdx]);
    case (Arrow_Left)  : return (LeftTextures[nIdx]);
    case (Arrow_Right) : return (RightTextures[nIdx]); 
    default :
        break;
    }
}

defaultproperties
{
	soundHitArrowLeft=sound'HPSounds.Magic_sfx.spell_lesson_HitArrow1'
    soundHitArrowRight=sound'HPSounds.Magic_sfx.spell_lesson_HitArrow2'
    soundHitArrowUp=sound'HPSounds.Magic_sfx.spell_lesson_HitArrow3'
    soundHitArrowDown=sound'HPSounds.Magic_sfx.spell_lesson_HitArrow4'
	soundMissed=sound'HPSounds.Magic_sfx.spell_dud'
	bHidden=true;
//	bFaceMoveDirection=false
	Style=STY_Translucent
        //Style=STY_Masked
	DrawScale=0.08
	DrawType=DT_MESH
    Mesh=SkeletalMesh'HProps.skSheetTestMesh'
    //bUnlit=True
    //StartDisplayAt=0
    MultiSkins(0)=Texture'SpellShapes.Shapes.SpAr_Lt_Idle'
    DirectionArrow=Arrow_Left
	// bStatic is false so SpellLessonTrigger can update rotation to rotation
	// of the lesson shape.
	bStatic=false		
    DirectionArrow(0)=Arrow_None
    DirectionArrow(1)=Arrow_None
    DirectionArrow(2)=Arrow_None
}
