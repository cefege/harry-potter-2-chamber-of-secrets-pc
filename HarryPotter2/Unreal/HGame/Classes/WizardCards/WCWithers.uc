//===============================================================================
// #21 Lord Stoddard Withers
//===============================================================================

class  WCWithers extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardWithersTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Witherssmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardWithersBigTexture FILE=TEXTURES\menu\Folio\Cards\Withersbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Lord Stoddard Withers";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=21
    skin(0)=Texture'HProps.skins.WizardCardWithersTex0'
	textureBig=Texture'WizCardWithersBigTexture'
	strDescriptionId="WizCard_0046"
}
