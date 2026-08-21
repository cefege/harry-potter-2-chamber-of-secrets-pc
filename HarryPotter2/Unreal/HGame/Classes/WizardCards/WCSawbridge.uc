//===============================================================================
// #26 Almerick Sawbridge
//===============================================================================

class  WCSawbridge extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardSawbridgeTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Sawbridgesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardSawbridgeBigTexture FILE=TEXTURES\menu\Folio\Cards\Sawbridgebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Almerick Sawbridge";
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=26
    skin(0)=Texture'HProps.skins.WizardCardSawbridgeTex0'
	textureBig=Texture'WizCardSawbridgeBigTexture'
	strDescriptionId="WizCard_0022"
}
