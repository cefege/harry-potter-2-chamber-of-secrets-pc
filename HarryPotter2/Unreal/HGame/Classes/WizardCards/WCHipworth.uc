//===============================================================================
// #58 Glover Hipworth
//===============================================================================

class  WCHipworth extends Bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardHipworthTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Hipworthsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardHipworthBigTexture FILE=TEXTURES\menu\Folio\Cards\Hipworthbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Glover Hipworth";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=58
    skin(0)=Texture'HProps.skins.WizardCardHipworthTex0'
	textureBig=Texture'WizCardHipworthBigTexture'
	strDescriptionId="WizCard_0094"
}
