//===============================================================================
//  #18 Ulric the Oddball
//===============================================================================

class  WCUlric extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardUricTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Uricsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardUricBigTexture FILE=TEXTURES\menu\Folio\Cards\Uricbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Uric the Oddball";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=18
    skin(0)=Texture'HProps.skins.WizardCardUricTex0'
	textureBig=Texture'WizCardUricBigTexture'
	strDescriptionId="WizCard_0056"
}
