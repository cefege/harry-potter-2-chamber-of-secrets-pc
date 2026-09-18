//===============================================================================
//  [EnemyHealthManager] 
//
//  The EnemyHealthManager handles the drawing of an enemy's health bar given
//  a reference to the enemy and a specification of which enemy health bar to
//  use.
//
//  To use the EnemyHealthManager
//
//  1) Spawn a EnemyHealthManager object from script.
//  2) Call object.Start(HChar) to begin display.
//  3) The HChar passed in needs to have a GetHealth() function that returns
//     a health value between 0 and 1.
//  4) The HChar passed in needs its EnemyHealthBar enum set to the right
//     enemy health bar.
//  5) When GetHealth() returns 0, display of the enemy health will stop.
//
//  Example of starting up EnemyHealthManager from script:
//
//      var EnemyHealthManager EHealth;
//
//     	EHealth = EnemyHealthManager(FancySpawn(class'EnemyHealthManager'));
//		EHealth.Start(self);
// 
//===============================================================================

class EnemyHealthManager extends HudItemManager;


// Names of enemy health textures
const strBAR_ARAGOG   = "HP_Menu.Hud.EnemyHealthAragog";   // Aragog bar
const strBAR_BASILISK = "HP_Menu.Hud.EnemyHealthBasilisk"; // Basilisk bar
const strBAR_DUELLIST = "HP_Menu.Hud.EnemyHealthWizard";   // Wizard Duelling bar
const strBAR_PEEVES   = "HP_Menu.Hud.EnemyHealthPeeves";   // Peeves bar
const strBAR_SEEKER   = "HP_Menu.Hud.EnemyHealthSeeker";   // Quidditch Seeker
const strBAR_EMPTY    = "HP_Menu.Hud.EnemyHealthEmpty";

const fBAR_W          = 116.0;   // Bar position within the graphic
const fBAR_H          = 20.0;
const fBAR_START_X    = 5;
const fBAR_START_Y    = 83;

const fSCREEN_X = 4;             // Screen position of enemy health

var texture textureBarFull;
var texture textureBarEmpty;
var HChar   Enemy;
var bool    bRegisteredWithHud;

// Dynamically load textures when manager created.
event PostBeginPlay()
{
	Super.PostBeginPlay();

	// Load emptybar
	textureBarEmpty = texture(DynamicLoadObject(strBAR_EMPTY, class'Texture'));
}

// Start display.
function Start(HChar EnemyIn)
{
	// Save off enemy (in order to call Enemy.GetHealth())
	Enemy = EnemyIn;

	// Setup the full bar to use.
	switch (EnemyIn.EnemyHealthBar)
	{
	case (EnemyIn.EEnemyBar.EnemyBar_Aragog) :
		textureBarFull = texture(DynamicLoadObject(strBAR_ARAGOG, class'Texture'));
		break;
	case (EnemyIn.EEnemyBar.EnemyBar_Basilisk) :
		textureBarFull = texture(DynamicLoadObject(strBAR_BASILISK, class'Texture'));
		break;
	case (EnemyIn.EEnemyBar.EnemyBar_Duellist) :
		textureBarFull = texture(DynamicLoadObject(strBAR_DUELLIST, class'Texture'));
		break;
	case (EnemyIn.EEnemyBar.EnemyBar_Peeves) :
		textureBarFull = texture(DynamicLoadObject(strBAR_PEEVES, class'Texture'));
		break;
	case (EnemyIn.EEnemyBar.EnemyBar_Seeker) :
		textureBarFull = texture(DynamicLoadObject(strBAR_SEEKER, class'Texture'));
		break;    
	default :
        log("ERROR: Missing enemy health enum");
        textureBarFull = texture(DynamicLoadObject(strBAR_DUELLIST, class'Texture'));
		break;
	}

	GoToState('DisplayEnemyHealth');
}

// Stop display.
function End()
{
	HPHud(Harry(Level.PlayerHarryActor).myHud).RegisterEnemyHealth(None);
    bRegisteredWithHud = false;

	GoToState('Idle');
}

//-----------------------------------------------------------------------------------
//  States
//-----------------------------------------------------------------------------------

//-----------------------------------------------------------------------------------
//  State Idle
//-----------------------------------------------------------------------------------

auto state idle
{
}

//-----------------------------------------------------------------------------------
//  State DisplayEnemyHealth
//-----------------------------------------------------------------------------------
//
//  Enemy health display is updated while in this state.

state DisplayEnemyHealth
{
	event Tick(float fDelta)
	{
		if (!bRegisteredWithHud)
		{
			if (level.PlayerHarryActor.myHud != None)
			{
				// Set things up so the hud will call us.
				HPHud(Harry(level.PlayerHarryActor).myHud).RegisterEnemyHealth(self);
				bRegisteredWithHud = true;
			}
		}
	}

	// RenderHudItemManager.  Called by Hud every render cycle.  
	function RenderHudItemManager(Canvas canvas, bool bMenuMode, bool bFullCutMode, bool bHalfCutMode)
	{
		local float fScaleFactor;		// amount to scale based on screen res
		local float fIconX;          // place enemy health icon here
		local float fIconY;
		local float fEnemyHealth;
		local float fEmptyHealth;
		local float fEmptyW;
		local float fBarEmptyOffset;

		local float fSegmentWidth;
		local float fSegmentStartAt;


		// Get scale for different screen resolutions.
		fScaleFactor = GetScaleFactor(canvas);

		// Display all of the full bar at left-bottom of screen.
		fIconX = fSCREEN_X * fScaleFactor;
		fIconY = canvas.SizeY - (fScaleFactor * textureBarFull.VSize);
		canvas.SetPos(fIconX,fIconY);
		canvas.DrawIcon(textureBarFull, fScaleFactor);

		// Get enemy health (0 to 1.0)
		fEnemyHealth = Enemy.GetHealth();
		fEnemyHealth = fclamp(fEnemyHealth, 0, 1.0);
		fEmptyHealth = 1.0 - fEnemyHealth;

		//Harry(level.PlayerHarryActor).ClientMessage("Health " $fEnemyHealth);
		fSegmentWidth = fEnemyHealth * (fBAR_W);
		Canvas.SetPos(fIconX + (fBAR_START_X * fScaleFactor), fIconY + (fBAR_START_Y * fScaleFactor));
		Canvas.DrawTile(textureBarEmpty,
						fSegmentWidth * fScaleFactor,
			            textureBarEmpty.VSize * fScaleFactor,
						0,
						0,
						fSegmentWidth,
						textureBarEmpty.VSize);

		if (fEnemyHealth <= 0.0)
			End();
	}
}


defaultproperties
{
	DrawType=DT_Sprite							
	bHidden=true                                
}

