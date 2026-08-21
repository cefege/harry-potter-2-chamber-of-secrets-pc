//===============================================================================
// #14 Fulbert the Fearful
//===============================================================================

class  WCFulbert extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardFulbertTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Fulbertsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardFulbertBigTexture FILE=TEXTURES\menu\Folio\Cards\fulbertbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Fulbert the Fearful";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=14
    skin(0)=Texture'HProps.skins.WizardCardFulbertTex0'
	textureBig=Texture'WizCardFulbertBigTexture'
	strDescriptionId="WizCard_0052"
}
