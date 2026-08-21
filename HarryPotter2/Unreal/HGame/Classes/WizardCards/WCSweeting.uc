//===============================================================================
//  #61 Havelock Sweeting
//===============================================================================

class  WCSweeting extends bronzecards;

#EXEC TEXTURE IMPORT NAME=WizardCardSweetingTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Sweetingsmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardSweetingBigTexture FILE=TEXTURES\menu\Folio\Cards\Sweetingbig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF


function PostBeginPlay()
{
	WizardName = "Havelock Sweeting";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=61
    skin(0)=Texture'HProps.skins.WizardCardSweetingTex0'
	textureBig=Texture'WizCardSweetingBigTexture'
	strDescriptionId="WizCard_0097"
}
