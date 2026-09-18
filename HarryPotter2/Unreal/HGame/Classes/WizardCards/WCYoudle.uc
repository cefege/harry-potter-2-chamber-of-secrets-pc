//===============================================================================
// #43	Cyprian Youdle
//===============================================================================

class  WCYoudle extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardYoudleTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Youdlesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardYoudleBigTexture FILE=TEXTURES\menu\Folio\Cards\Youdlebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Cyprian Youdle";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=43
    skin(0)=Texture'HProps.skins.WizardCardYoudleTex0'
	textureBig=Texture'WizCardYoudleBigTexture'
	strDescriptionId="WizCard_0081"
}
