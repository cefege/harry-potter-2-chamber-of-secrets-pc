//===============================================================================
// #88 Celestina Warbeck
//===============================================================================

class  WCWarbeck extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardWarbeckTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Warbecksmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardWarbeckBigTexture FILE=TEXTURES\menu\Folio\Cards\Warbeckbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Celestina Warbeck";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=88
    skin(0)=Texture'HProps.skins.WizardCardWarbeckTex0'
	textureBig=Texture'WizCardWarbeckBigTexture'
	strDescriptionId="WizCard_0002"
}
