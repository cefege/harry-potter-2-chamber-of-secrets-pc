//===============================================================================
// #52	Felix Summerbee
//===============================================================================

class  WCSummerbee extends Silvercards;

#EXEC TEXTURE IMPORT NAME=WizardCardSummerbeeTex0  FILE=..\HGame\Textures\Menu\Folio\Cards\Summerbeesmall.bmp  GROUP=Skins
#EXEC TEXTURE IMPORT NAME=WizCardSummerbeeBigTexture FILE=TEXTURES\menu\Folio\Cards\Summerbeebig.bmp GROUP="Icons" FLAGS=2 MIPS=OFF

function PostBeginPlay()
{
	WizardName = "Felix Summerbee";
	
	Super.PostBeginPlay();
}

defaultproperties
{
	ID=52
    skin(0)=Texture'HProps.skins.WizardCardSummerbeeTex0'
	textureBig=Texture'WizCardSummerbeeBigTexture'
	strDescriptionId="WizCard_0035"
}
