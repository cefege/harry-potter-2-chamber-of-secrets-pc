//===============================================================================
//  [JarBeans] 
//===============================================================================

class JarBeans extends HBottlesJars;

var int         nAddBeans;
var string      strCueAddBeansDone;
var Jellybean   NewJellybean;
var vector      vTopOfJar;
var StatusGroup sgJBeans;

// Called from CutScene script.
function bool CutCommand(string command, optional string cue, optional bool bFastFlag)
{
	local string  sActualCommand;
	local string  sCutName;
	local actor   a;
	
	sActualCommand = ParseDelimitedString( command, " ", 1, false );


	if( sActualCommand ~= "Capture" )
	{
		return (true);
	}

	else 
	if( sActualCommand ~= "Release" )
	{
		return (true);
	}

	else
	if( sActualCommand ~= "AddBeans" )
	{
        // Used to pass beans to add to hud as a parameter, but this command is for
        // a "Harry won" dueling cutscene and we always want to add the number
        // of beans that Harry wagered * 2.
        //nAddBeans = int(ParseDelimitedString(command," ",2,false));
        nAddBeans = 2 * playerHarry.DuelRankBeans;
        strCueAddBeansDone = cue;
        GoToState('GiveHarryBeans');
		return (true);
	}

	else
		return Super.CutCommand( Command, Cue, bFastFlag );
}

auto state Idle
{
}

state GiveHarryBeans
{

begin: 
    
    vTopOfJar = Location;
    vTopOfJar.Z += CollisionHeight;

    if (nAddBeans > 0)
    {
        sgJBeans = playerHarry.managerStatus.GetStatusGroup(class'StatusGroupJellybeans');

    	sgJBeans.SetEffectTypeToPermanent();
        sgJBeans.SetCutSceneRenderMode(true);

        while (nAddBeans > 0)
        {
            NewJellybean = Jellybean(FancySpawn(class'Jellybean',,,vTopOfJar));
            SetPhysics(PHYS_Walking);
            NewJellybean.DoPickupProp();
            --nAddBeans;
            sleep(0.1);
        }

        sleep(0.5);

    	sgJBeans.SetEffectTypeToNormal();
        sgJBeans.SetCutSceneRenderModeToNormal();
    }

    if (strCueAddBeansDone != "")
		Super.CutCue(strCueAddBeansDone);

    GoToState('Idle');
}

defaultproperties
{
     Mesh=SkeletalMesh'HProps.skJarBeansMesh'
     DrawScale=2.5
     CollisionRadius=18
     CollisionHeight=32
 	 CutName="JarBeans"
}
