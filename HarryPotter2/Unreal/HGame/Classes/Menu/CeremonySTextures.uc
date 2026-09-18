//===============================================================================
//  [CeremonySTextures] 
//
//  Placing a CeremonySTextures object in a level will cause textures
//  assigned to stexGryffindor, stexSlytherin, stexHufflepuff and stexRavenclaw
//  to appear with housepoint score text whereever they are used in the level.
//
//  Steps for adding housepoint scripted textures and using them in
//  a level:
//
//    For each house, add a regualar version of the texture that will display
//    the housepoint score and a scripted texture version.
//
//			1) Import a .pcx graphic into the Texture Browser as usual.
//			2) From the Texture Browser, go to File New to add a scripted 
//			   version of the texture.
//			3) Select the Package that corresponds to the texture imported in 
//             step 1.
//			4) Give the scripted version a name that identifies it as a scripted
//             version of the texture from step 1.
//			5) Select ScriptedTexture in the Class combo box.
//			6) Set the width and height to be the same as the texture from step 1.
//			7) Click Ok.
//			8) In the ScriptedTexture property, set SourceTexture to the texture
//             from Step 1.
//
//    Place the housepoint ScriptedTextures in the level:
//
//          9) For each housepoint scripted texture, select a surface and apply
//             the scripted texture to that surface.
//
//    Add a CeremonySTextures object to the level and setup the properties:
//
//         10) Place a CeremonySTextures object anywhere in the level.
//         11) Assign the housepoint scripted texture objects to stexGryffindor,
//             stexSlytherin, stexHufflepuff, and stexRavenclaw.
//         12) Set nScoreMidx and nScoreMidY to be the offset within the texture
//             that you want to center the score text around.  It is assumed that 
//             all 4 houses will have the same general design and therefore
//             use the same score position.  This can be changed if that ever
//             turns out not to be the case.
//         13) Set ScoreFontSize to one of the EScoreFont enum values.
//         14) Set colorScoreFont to the desired color to be used for drawing
//             the score.
//  
//===============================================================================

class CeremonySTextures expands Info;

// Font size to use for the score.
enum EScoreFont
{
	ScoreFont_Tiny,
	ScoreFont_Small,
	ScoreFont_Medium,
	ScoreFont_Big
};

// Designer customizations.
var() EScoreFont ScoreFontSize;     // Score font size
var() Color      colorScoreFont;    // Color of score text
var() int        nScoreMidX;        // Score centered horizontally about this pos
var() int        nScoreMidY;        // Score centered vertically about this pos
var() Texture    stexGryffindor;    // When these are assigned in the editor, make
var() Texture    stexSlytherin;     // sure scripted textures are selected from the  
var() Texture    stexHufflepuff;    // Texture Browser.  Even though this class 
var() Texture    stexRavenclaw;     // requires that scripted textures be used, 
                                    // the type specified here has to be texture
                                    // or you won't be able to select from the
                                    // Texture Browser in the editor.

// This timer is set in BeginPlay and keeps getting called until we can find Harry 
// Harry isn't loaded when this object's BeginPlay is called and we need to do
// some stuff that relies on Harry.  After Harry has been found, we do our stuff
// and the timer is turned off.
function Timer()
{
	local Harry                  playerHarry;
	local StatusGroupHousePoints sgHousePts;

	playerHarry = Harry(Level.PlayerHarryActor);

	// Found Harry
	if (playerHarry != None)
	{
		// Turn off timer
		SetTimer(0.0, false);

		// Resolve any ties between houses.  We do this because when a ceremony happens,
		// there are no cutscenes to deal with ties between 2 houses.  So, we artificially
		// adjust housepoints so there are no ties.
		sgHousePts = StatusGroupHousePoints(playerHarry.managerStatus.GetStatusGroup
			                                           (class'StatusGroupHousePoints'));
		sgHousePts.ResolveTies();
	}
}

// Sets the notify actor propery of each scripted texture this class so that it will
// recieve RenderTexture() events from it.
function BeginPlay()
{
	if (stexGryffindor == None ||
		stexSlytherin  == None ||
        stexHufflepuff == None ||		
		stexRavenclaw  == None)
	{
		log("WARNING: One or more CeremonySTexture house textures are none");
	}

	// Setup NotifyActor for each house's ScriptedTexture.
	SetSTexturesNotifyActor(Self);

	// Start a timer that will allow us to initialize some stuff after
	// Harry has been created.
	SetTimer(0.1, true);
}

// Clears the NotifyActor property of the scripted textures when this actor is destoyed.
function Destroyed()
{
	SetSTexturesNotifyActor(None);
}


// Called when a housepoint texture is rendered during the game.
event RenderTexture(ScriptedTexture Tex)
{
	local StatusGroup sgHousePts;				// Status group for housepoint totals
	local font        fontScore;                // Font to use for score display
	local int         nPoints;                  // Points to display
	local string      strPoints;                // String form of points
	local float       fScoreXLen, fScoreYLen;   // Drawing size of points string
	local int         nScoreXPos, nScoreYPos;   // Position of points string on texture
	local baseConsole ConsoleForFont;           // Get font from the console object
	
	// Set font to use based on enum setup by designer.
	ConsoleForFont = baseConsole(Harry(Level.PlayerHarryActor).player.console);
	if (ConsoleForFont == None)
		Level.PlayerHarryActor.ClientMessage("WARNING: CeremonySTexture could not get font");
	switch (ScoreFontSize)
	{
	case (ScoreFont_Tiny)   :
		fontScore = ConsoleForFont.LocalTinyFont;
		break;
	case (ScoreFont_Small)  :
		fontScore = ConsoleForFont.LocalSmallFont;
		break;
	case (ScoreFont_Medium) :
		fontScore = ConsoleForFont.LocalMedFont;
		break;
	case (ScoreFont_Big)    :
		fontScore = ConsoleForFont.LocalBigFont;
		break;
	default :
		Level.PlayerHarryActor.ClientMessage("WARNING: Font enum setup problem in CeremonySTextures");
		break;
	}

	// Get score that goes with the house texture passed in.
	sgHousePts = Harry(Level.PlayerHarryActor).
		         managerStatus.GetStatusGroup(class'StatusGroupHousePoints');
	if (Tex == stexGryffindor)
		nPoints = sgHousePts.GetStatusItem(class'StatusItemGryffindorPts').nCount;
	else if (Tex == stexSlytherin)
		nPoints = sgHousePts.GetStatusItem(class'StatusItemSlytherinPts').nCount;
	else if (Tex == stexHufflepuff)
		nPoints = sgHousePts.GetStatusItem(class'StatusItemHufflepuffPts').nCount;
	else if (Tex == stexRavenclaw)
		nPoints = sgHousePts.GetStatusItem(class'StatusItemRavenclawPts').nCount;
	else
		Level.PlayerHarryActor.ClientMessage("WARNING: Unexpected scripted texture in CeremonySTextures");

	// Convert points to a string and figure out where on the texture the
	// string should be placed.
	strPoints = string(nPoints);
	Tex.TextSize(strPoints, fScoreXLen, fScoreYLen, fontScore);
	nScoreXPos = nScoreMidX - fScoreXLen/2;
	nScoreYPos = nScoreMidY - fScoreYLen/2;

	// Draw the score on the texture.
	Tex.DrawColoredText( nScoreXPos, nScoreYPos, strPoints, fontScore, colorScoreFont);	
}

// Set or clear each scripted texture's NotifyActor property.
function SetSTexturesNotifyActor(Actor aSet)
{
	if (stexGryffindor != None)
		ScriptedTexture(stexGryffindor).NotifyActor = aSet;
	if (stexSlytherin != None)
		ScriptedTexture(stexSlytherin).NotifyActor = aSet;
	if (stexHufflepuff != None)
		ScriptedTexture(stexHufflepuff).NotifyActor = aSet;
	if (stexRavenclaw != None)
		ScriptedTexture(stexRavenclaw).NotifyActor = aSet;
}

defaultproperties
{
	bNoDelete=True
	bAlwaysRelevant=True
	ScoreFontSize=ScoreFont_Medium
	nScoreMidX=65
	nScoreMidY=87
}

